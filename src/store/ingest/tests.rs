//! Publisher equality skips derived work while real changes retain owned state.

use super::*;
use crate::model::{
    ArticleQuery, ArticleTarget, ExtractedDocument, FeedOrigin, View,
};
use anyhow::Context;

struct Fixture {
    directory: tempfile::TempDir,
    store: Store,
    feed: Feed,
    articles: Vec<IncomingArticle>,
}

impl Fixture {
    fn new() -> Result<Self> {
        let directory = tempfile::tempdir()?;
        let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
        store.subscribe("https://journal.example/feed")?;
        let feed = store.feeds()?.remove(0);
        let parsed = crate::source::parse(
            include_bytes!("../../../tests/fixtures/rss.xml"),
            &feed.url,
        )?;
        store.ingest(&feed, &parsed.title, &parsed.articles)?;
        let feed = store.feeds()?.remove(0);
        Ok(Self {
            directory,
            store,
            feed,
            articles: parsed.articles,
        })
    }

    fn refresh(&mut self) -> Result<()> {
        self.store
            .ingest(&self.feed, &self.feed.title, &self.articles)
    }

    fn search(&self, text: &str) -> Result<usize> {
        Ok(self
            .store
            .query_articles(&ArticleQuery {
                view: View::All,
                text: text.into(),
                offset: 0,
                limit: 10,
                retained: None,
            })?
            .total)
    }
}

#[test]
fn unchanged_refresh_is_write_free_and_keeps_full_cache_and_personal_state()
-> Result<()> {
    let mut fixture = Fixture::new()?;
    fixture.store.set_starred(1, true)?;
    fixture.store.save_comment(1, None, "Keep my note")?;
    let target = ArticleTarget {
        article: 1,
        url: fixture.articles[0].url.clone(),
    };
    fixture.store.cache_document(
        &target,
        &ExtractedDocument {
            final_url: target.url.clone(),
            body: "Independent quartzcache text. ".repeat(100),
        },
    )?;
    fixture.store.save_progress(1, 12)?;
    fixture.store.save_document_progress(1, 1, 24)?;
    let observer =
        Store::open(&fixture.directory.path().join("library.sqlite3"))?;
    let version = observer.retention_version()?;
    let before = fixture.store.connection.total_changes();
    fixture.refresh()?;
    assert_eq!(fixture.store.connection.total_changes(), before);
    assert_eq!(observer.retention_version()?, version);
    assert!(fixture.store.article(1)?.starred);
    assert_eq!(fixture.store.comments(1)?[0].body, "Keep my note");
    assert_eq!(fixture.store.progress(1)?, 12);
    let full = fixture.store.cached_document(1)?.context("Full cache")?;
    assert_eq!((full.revision, full.progress), (1, 24));
    assert_eq!(fixture.search("quartzcache")?, 1);
    Ok(())
}

#[test]
fn publisher_corrections_replace_search_and_all_metadata_without_resetting_progress()
-> Result<()> {
    let mut fixture = Fixture::new()?;
    fixture.store.save_progress(1, 12)?;
    let original = fixture.articles[0].body.clone();
    fixture.articles[0].body = "New heliocentric publisher correction.".into();
    fixture.refresh()?;
    assert_eq!(fixture.search("heliocentric")?, 1);
    fixture.articles[0].title = "Corrected title".into();
    fixture.refresh()?;
    fixture.articles[0].author = "Corrected author".into();
    fixture.refresh()?;
    fixture.articles[0].published += 100;
    fixture.refresh()?;
    fixture.articles[0].url = "https://journal.example/moved".into();
    fixture.refresh()?;
    fixture.articles[0].body_origin = FeedOrigin::Summary;
    fixture.refresh()?;
    let actual = fixture.store.article(1)?;
    assert_eq!(actual.title, "Corrected title");
    assert_eq!(actual.author, "Corrected author");
    assert_eq!(actual.published, fixture.articles[0].published);
    assert_eq!(actual.url, fixture.articles[0].url);
    assert_eq!(fixture.store.feed_origin(1)?, FeedOrigin::Summary);
    assert_ne!(fixture.store.body(1)?, original);
    assert_eq!(fixture.store.progress(1)?, 12);
    fixture.articles[0].body = "Replacement obeliskword only.".into();
    fixture.refresh()?;
    assert_eq!(fixture.search("heliocentric")?, 0);
    assert_eq!(fixture.search("obeliskword")?, 1);
    Ok(())
}

#[test]
fn unchanged_articles_still_allow_validator_title_and_health_updates()
-> Result<()> {
    let mut fixture = Fixture::new()?;
    fixture.store.feed_error(fixture.feed.id, "HTTP 503")?;
    fixture.feed.etag = "new-etag".into();
    fixture.feed.modified = "Wed, 21 Oct 2026 07:28:00 GMT".into();
    fixture.feed.title = "Renamed publisher".into();
    fixture.refresh()?;
    let saved = fixture.store.feeds()?.remove(0);
    assert_eq!(saved.etag, fixture.feed.etag);
    assert_eq!(saved.modified, fixture.feed.modified);
    assert!(saved.error.is_empty());
    assert_eq!(saved.title, fixture.feed.title);
    assert_eq!(fixture.search("Renamed publisher")?, 1);
    let before = fixture.store.connection.total_changes();
    fixture.refresh()?;
    fixture.store.feed_error(fixture.feed.id, "")?;
    assert_eq!(fixture.store.connection.total_changes(), before);
    Ok(())
}
