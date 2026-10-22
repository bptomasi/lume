//! Interactive relevance orders the entire match set before pagination.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus, Mode},
    model::{ArticleQuery, FeedOrigin, IncomingArticle, SmartFilter, View},
    store::Store,
};

fn library() -> Result<(tempfile::TempDir, Store)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("relevance.sqlite3"))?;
    store.subscribe("https://journal.example/feed")?;
    let feed = store.feeds()?.remove(0);
    let titles = [
        "Why Is My Kubernetes Job Still Pending?",
        "Tracing: Why Is My Kubernetes Job Still Pending?",
        "Kubernetes: Why My Job Is Pending",
        "Cutting our inference bill",
        "What's in the SOSS?",
    ];
    let articles = titles
        .iter()
        .enumerate()
        .map(|(index, title)| IncomingArticle {
            external_id: index.to_string(),
            title: (*title).into(),
            url: format!("https://journal.example/{index}"),
            author: "Journal author".into(),
            published: 1_790_000_000 + index as i64,
            body: "Why is my Kubernetes job pending? A technical guide.".into(),
            body_origin: FeedOrigin::Feed,
        })
        .collect::<Vec<_>>();
    store.ingest(&feed, "Journal", &articles)?;
    Ok((directory, store))
}

#[test]
fn live_background_search_selects_the_best_title_before_enter() -> Result<()> {
    let (directory, store) = library()?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(
        &directory.path().join("relevance.sqlite3"),
    )?;
    app.focus = Focus::Articles;
    app.key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE))?;
    app.paste("no-such-article")?;
    app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL))?;
    for character in "Why Is My Kubernetes".chars() {
        app.key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))?;
    }
    settle_navigation(&mut app)?;
    assert!(matches!(app.mode, Mode::Input(_)));
    assert_eq!(app.page_total, 5);
    assert_eq!(
        app.current().map(|article| article.title.as_str()),
        Some("Why Is My Kubernetes Job Still Pending?")
    );
    app.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))?;
    settle_navigation(&mut app)?;
    let selected = app.current().map(|article| article.id);
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    settle_navigation(&mut app)?;
    assert_eq!(app.current().map(|article| article.id), selected);
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    settle_navigation(&mut app)?;
    assert!(app.search.is_empty());
    assert_eq!(
        app.library
            .articles
            .first()
            .map(|article| article.title.as_str()),
        Some("What's in the SOSS?")
    );
    app.end_session()?;
    Ok(())
}

fn settle_navigation(app: &mut App) -> Result<()> {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        app.poll_navigation()?;
        if !app.navigation_loading {
            return Ok(());
        }
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "Live relevance query timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn relevance_never_relaxes_boolean_filters_or_note_only_membership()
-> Result<()> {
    let (_directory, store) = library()?;
    let first = store
        .query_articles(&query("Why Is My Kubernetes"))?
        .articles
        .remove(0);
    store.save_comment(first.id, None, "Semaphore recovery")?;
    store.set_unread(first.id, false)?;
    for text in ["notes:semaphore", "semaphore", "title:pending AND is:read"] {
        let page = store.query_articles(&query(text))?;
        assert_eq!(page.total, 1, "{text}");
        assert_eq!(page.articles[0].id, first.id, "{text}");
    }
    let page = store.query_articles(&query(
        "(title:pending OR notes:semaphore) AND NOT title:tracing",
    ))?;
    assert_eq!(page.total, 2);
    assert!(
        page.articles
            .iter()
            .all(|article| !article.title.starts_with("Tracing"))
    );
    assert!(
        store
            .query_articles(&query("body:\"x' OR 1=1 --\""))?
            .articles
            .is_empty()
    );
    Ok(())
}

#[test]
fn unicode_title_prefixes_use_the_same_tokenizer_as_membership() -> Result<()> {
    let (_directory, mut store) = library()?;
    let feed = store.feeds()?.remove(0);
    store.ingest(
        &feed,
        "Journal",
        &[
            IncomingArticle {
                external_id: "unicode-title".into(),
                title: "ÉTUDE: Über Kubernetes".into(),
                url: "https://journal.example/unicode".into(),
                author: String::new(),
                published: 1,
                body: "A guide.".into(),
                body_origin: FeedOrigin::Feed,
            },
            IncomingArticle {
                external_id: "unicode-body".into(),
                title: "New guide".into(),
                url: "https://journal.example/new-guide".into(),
                author: String::new(),
                published: 2,
                body: "Étude über Kubernetes.".into(),
                body_origin: FeedOrigin::Feed,
            },
        ],
    )?;
    for text in ["étude über", "ETUDE UBER", "title:\"étude über\""] {
        let page = store.query_articles(&query(text))?;
        assert_eq!(page.articles[0].title, "ÉTUDE: Über Kubernetes", "{text}");
    }
    Ok(())
}

#[test]
fn ranking_preserves_destination_scopes_and_excludes_trashed_matches()
-> Result<()> {
    let (_directory, store) = library()?;
    let ordered = store
        .query_articles(&query("Why Is My Kubernetes"))?
        .articles;
    let collection = store.create_collection("Selected sources")?;
    store.add_member(collection, ordered[3].id)?;
    let mut request = query("Why Is My Kubernetes");
    request.view = View::Collection(collection);
    let page = store.query_articles(&request)?;
    assert_eq!(page.total, 1);
    assert_eq!(page.articles[0].id, ordered[3].id);
    store.set_deleted(ordered[0].id, true)?;
    request.view = View::All;
    assert_eq!(
        store.query_articles(&request)?.articles[0].id,
        ordered[1].id
    );
    assert!(store.page_containing(&request, ordered[0].id)?.is_none());
    request.view = View::Trash;
    let page = store.query_articles(&request)?;
    assert_eq!(page.total, 1);
    assert_eq!(page.articles[0].id, ordered[0].id);
    Ok(())
}

#[test]
fn equal_relevance_and_dates_have_a_stable_identity_tie_breaker() -> Result<()>
{
    let (_directory, mut store) = library()?;
    let feed = store.feeds()?.remove(0);
    let entries = (0..3)
        .map(|index| IncomingArticle {
            external_id: format!("tie-{index}"),
            title: "Semaphore internals".into(),
            url: format!("https://journal.example/tie-{index}"),
            author: String::new(),
            published: 1,
            body: "An identical document.".into(),
            body_origin: FeedOrigin::Feed,
        })
        .collect::<Vec<_>>();
    store.ingest(&feed, "Journal", &entries)?;
    let page = store.query_articles(&query("semaphore"))?;
    assert_eq!(page.total, 3);
    assert!(page.articles.windows(2).all(|pair| pair[0].id > pair[1].id));
    Ok(())
}

fn query(text: &str) -> ArticleQuery {
    ArticleQuery {
        view: View::All,
        text: text.into(),
        offset: 0,
        limit: 100,
        retained: None,
    }
}

#[test]
fn title_prefix_then_phrase_then_title_terms_outrank_newer_body_matches()
-> Result<()> {
    let (_directory, store) = library()?;
    for text in ["Why Is My Kubernetes", "why is my kuber"] {
        let page = store.query_articles(&query(text))?;
        assert_eq!(page.total, 5);
        assert_eq!(
            page.articles[0].title,
            "Why Is My Kubernetes Job Still Pending?"
        );
        assert_eq!(
            page.articles[1].title,
            "Tracing: Why Is My Kubernetes Job Still Pending?"
        );
        assert_eq!(page.articles[2].title, "Kubernetes: Why My Job Is Pending");
    }
    Ok(())
}

#[test]
fn relevance_is_global_and_anchor_pages_use_the_identical_order() -> Result<()>
{
    let (_directory, store) = library()?;
    let mut request = query("Why Is My Kubernetes");
    let expected = store.query_articles(&request)?.articles;
    request.limit = 2;
    for (position, article) in expected.iter().enumerate() {
        let (offset, page) = store
            .page_containing(&request, article.id)?
            .ok_or_else(|| anyhow::anyhow!("Ranked target disappeared"))?;
        assert_eq!(offset, position / 2 * 2);
        assert_eq!(page.total, 5);
        assert_eq!(page.articles[position % 2].id, article.id);
        request.offset = position;
        assert_eq!(store.query_articles(&request)?.articles[0].id, article.id);
    }
    Ok(())
}

#[test]
fn saved_lists_keep_date_order_until_an_interactive_query_is_entered()
-> Result<()> {
    let (_directory, store) = library()?;
    store.save_smart_list(
        "Kubernetes",
        &SmartFilter {
            text: "kubernetes".into(),
            ..SmartFilter::default()
        },
    )?;
    let mut request = query("");
    request.view = View::Smart(store.smart_lists()?.remove(0).id);
    assert_eq!(
        store.query_articles(&request)?.articles[0].title,
        "What's in the SOSS?"
    );
    request.text = "Why Is My Kubernetes".into();
    assert_eq!(
        store.query_articles(&request)?.articles[0].title,
        "Why Is My Kubernetes Job Still Pending?"
    );
    request.text = "is:unread".into();
    assert_eq!(
        store.query_articles(&request)?.articles[0].title,
        "What's in the SOSS?"
    );
    Ok(())
}
