//! Boolean search and saved lists share one validated, parameterized contract.

use anyhow::{Result, ensure};
use lume::{
    model::{
        ArticleQuery, FeedOrigin, IncomingArticle, SmartFilter, TagTarget, View,
    },
    search,
    store::Store,
};

fn library() -> Result<(tempfile::TempDir, Store)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("search.sqlite3"))?;
    store.subscribe("https://journal.example/feed")?;
    let feed = store.feeds()?.remove(0);
    let entries = fixtures();
    store.ingest(&feed, "Journal", &entries)?;
    Ok((directory, store))
}

fn titles(store: &Store, text: &str, view: View) -> Result<Vec<String>> {
    let page = store.query_articles(&ArticleQuery {
        view,
        text: text.into(),
        offset: 0,
        limit: 100,
        retained: None,
    })?;
    ensure!(
        page.total == page.articles.len(),
        "Test fixture unexpectedly paginated"
    );
    Ok(page
        .articles
        .into_iter()
        .map(|article| article.title)
        .collect())
}

#[test]
fn bounded_pages_keep_full_counts_order_and_their_own_annotation_metadata()
-> Result<()> {
    let (_directory, store) = library()?;
    let annotated = store
        .library()?
        .articles
        .into_iter()
        .find(|article| article.title == "eBPF internals")
        .ok_or_else(|| anyhow::anyhow!("Missing annotated fixture"))?;
    store.save_comment(annotated.id, None, "Keep this explanation")?;

    for (offset, title) in
        ["eBPF internals", "Kubernetes webinar", "Kubernetes Cilium"]
            .into_iter()
            .enumerate()
    {
        let page = store.query_articles(&ArticleQuery {
            view: View::All,
            text: "topic:kubernetes OR topic:ebpf".into(),
            offset,
            limit: 1,
            retained: None,
        })?;
        assert_eq!(page.total, 3);
        assert_eq!(page.articles.len(), 1);
        assert_eq!(page.articles[0].title, title);
        assert_eq!(
            page.articles[0].comments,
            usize::from(title == "eBPF internals")
        );
    }
    let beyond = store.query_articles(&ArticleQuery {
        view: View::All,
        text: "topic:kubernetes OR topic:ebpf".into(),
        offset: 3,
        limit: 1,
        retained: None,
    })?;
    assert_eq!(beyond.total, 3);
    assert!(beyond.articles.is_empty());
    Ok(())
}

#[test]
fn boolean_fields_and_saved_searches_return_independent_expected_results()
-> Result<()> {
    let (_directory, store) = library()?;
    let text =
        "(topic:kubernetes OR topic:ebpf) AND is:unread AND NOT title:webinar";
    assert_eq!(
        titles(&store, text, View::All)?,
        ["eBPF internals", "Kubernetes Cilium"]
    );
    store.save_smart_list(
        "Deep systems",
        &SmartFilter {
            text: text.into(),
            ..SmartFilter::default()
        },
    )?;
    let saved = store.smart_lists()?.remove(0);
    assert_eq!(
        titles(&store, "", View::Smart(saved.id))?,
        ["eBPF internals", "Kubernetes Cilium"]
    );
    assert_eq!(
        titles(&store, "author:Grace", View::Smart(saved.id))?,
        ["eBPF internals"]
    );
    assert_eq!(
        titles(&store, "domain:infra.example AND minutes:2..10", View::All)?,
        ["Kubernetes Cilium"]
    );
    Ok(())
}

#[test]
fn phrases_prefixes_and_boolean_precedence_remain_distinct() -> Result<()> {
    let (_directory, store) = library()?;
    assert_eq!(
        titles(&store, "\"kernel programming\"", View::All)?,
        ["Kubernetes Cilium"]
    );
    assert_eq!(
        titles(&store, "title:kuber", View::All)?,
        ["Kubernetes webinar", "Kubernetes Cilium"]
    );
    assert_eq!(
        titles(
            &store,
            "author:Grace OR author:Ada AND title:webinar",
            View::All
        )?,
        ["Kubernetes webinar", "eBPF internals"]
    );
    Ok(())
}

#[test]
fn invalid_or_injected_queries_never_become_unrestricted_results() -> Result<()>
{
    let (_directory, store) = library()?;
    for query in [
        "title:",
        "unknown:value",
        "is:maybe",
        "after:tomorrow",
        "minutes:9..1",
        "rust OR",
        "(rust",
        "\"unfinished",
    ] {
        assert!(titles(&store, query, View::All).is_err(), "{query}");
        assert!(
            store
                .save_smart_list(
                    "Invalid",
                    &SmartFilter {
                        text: query.into(),
                        ..SmartFilter::default()
                    }
                )
                .is_err()
        );
    }
    assert!(titles(&store, "title:\"x' OR 1=1 --\"", View::All)?.is_empty());
    assert!(
        search::parse(&format!("{}rust{}", "(".repeat(17), ")".repeat(17)))
            .is_err()
    );
    assert!(search::parse(&"rust ".repeat(257)).is_err());
    assert_eq!(search::suggestions("auth"), ["author:"]);
    Ok(())
}

fn fixtures() -> Vec<IncomingArticle> {
    CASES
        .iter()
        .enumerate()
        .map(|(index, case)| IncomingArticle {
            external_id: index.to_string(),
            title: case.title.into(),
            url: format!("https://{}/article/{index}", case.domain),
            author: case.author.into(),
            published: 1_791_072_000 + case.day * 86_400,
            body: case.body.repeat(case.repetitions),
            body_origin: FeedOrigin::Feed,
        })
        .collect()
}

struct SearchCase {
    title: &'static str,
    author: &'static str,
    domain: &'static str,
    day: i64,
    body: &'static str,
    repetitions: usize,
}

const CASES: &[SearchCase] = &[
    SearchCase {
        title: "Kubernetes Cilium",
        author: "Ada",
        domain: "infra.example",
        day: 1,
        body: "kernel programming ",
        repetitions: 120,
    },
    SearchCase {
        title: "eBPF internals",
        author: "Grace",
        domain: "kernel.example",
        day: 2,
        body: "A kernel guide for safe packet processing.",
        repetitions: 1,
    },
    SearchCase {
        title: "Kubernetes webinar",
        author: "Ada",
        domain: "infra.example",
        day: 3,
        body: "A short announcement about a talk.",
        repetitions: 1,
    },
];

#[test]
fn date_boundaries_include_start_and_exclude_end_at_utc_midnight() -> Result<()>
{
    let (_directory, store) = library()?;
    let article = store
        .library()?
        .articles
        .into_iter()
        .find(|article| article.title == "eBPF internals")
        .ok_or_else(|| anyhow::anyhow!("Fixture missing"))?;
    let exact_day = chrono::DateTime::from_timestamp(article.published, 0)
        .ok_or_else(|| anyhow::anyhow!("Invalid fixture time"))?
        .format("%Y-%m-%d")
        .to_string();
    assert_eq!(
        titles(
            &store,
            &format!("after:{exact_day} AND author:Grace"),
            View::All
        )?,
        ["eBPF internals"]
    );
    assert!(
        titles(
            &store,
            &format!("before:{exact_day} AND author:Grace"),
            View::All
        )?
        .is_empty()
    );
    Ok(())
}

#[test]
fn article_and_highlight_tag_filters_have_independent_owners() -> Result<()> {
    let (_directory, store) = library()?;
    let article = store
        .library()?
        .articles
        .into_iter()
        .find(|article| article.title == "eBPF internals")
        .ok_or_else(|| anyhow::anyhow!("Fixture missing"))?;
    store.set_tags(TagTarget::Article(article.id), &["networking".into()])?;
    assert_eq!(
        titles(&store, "tag:networking", View::All)?,
        ["eBPF internals"]
    );
    store.save_highlight(
        article.id,
        "A kernel",
        0,
        "A useful systems concept",
    )?;
    let comment = store.comments(article.id)?[0].id;
    store.set_tags(TagTarget::Comment(comment), &["consensus".into()])?;
    assert_eq!(
        titles(&store, "highlight-tag:consensus", View::All)?,
        ["eBPF internals"]
    );
    assert!(titles(&store, "tag:consensus", View::All)?.is_empty());
    Ok(())
}

#[test]
fn domain_matches_normalized_hostname_with_ports_and_excludes_host_suffixes()
-> Result<()> {
    let (_directory, mut store) = library()?;
    let feed = store.feeds()?.remove(0);
    let mut entries = fixtures();
    entries[0].url = "https://INFRA.example:8443/article#details".into();
    entries[1].url = "https://infra.example.evil.test/article".into();
    store.ingest(&feed, "Journal", &entries)?;

    assert_eq!(
        titles(&store, "domain:infra.example", View::All)?,
        ["Kubernetes webinar", "Kubernetes Cilium"]
    );
    assert!(titles(&store, "domain:infra.example:8443", View::All).is_err());
    Ok(())
}

#[test]
fn anchored_pages_use_the_same_filters_and_never_escape_into_another_view()
-> Result<()> {
    let (_directory, store) = library()?;
    let target = store.library()?.articles.remove(2).id;
    let mut query = ArticleQuery {
        view: View::All,
        text: "author:Ada".into(),
        offset: 0,
        limit: 1,
        retained: None,
    };

    let (offset, page) = store
        .page_containing(&query, target)?
        .ok_or_else(|| anyhow::anyhow!("Matching target was lost"))?;
    assert_eq!((offset, page.total), (1, 2));
    assert_eq!(page.articles[0].title, "Kubernetes Cilium");
    query.text = "author:Grace".into();
    assert!(store.page_containing(&query, target)?.is_none());
    query.text.clear();
    store.set_deleted(target, true)?;
    assert!(store.page_containing(&query, target)?.is_none());
    query.view = View::Trash;
    assert!(store.page_containing(&query, target)?.is_some());
    Ok(())
}
