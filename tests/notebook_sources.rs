//! Narrow note reads retain live source, annotation and deletion contracts.

use anyhow::{Context, Result};
use lume::{
    model::{
        ArticleTarget, ContentSource, ExtractedDocument, Feed, FeedOrigin,
        IncomingArticle, NoteKind, NoteMetadata, Passage, TagTarget,
    },
    store::Store,
};

const BODY: &str = "Publisher text: café, 界 and a durable mechanism.\n";

fn incoming(url: &str) -> IncomingArticle {
    IncomingArticle {
        external_id: "source".into(),
        title: "Source identity".into(),
        url: url.into(),
        body: BODY.repeat(40),
        body_origin: FeedOrigin::Feed,
        author: "Writer".into(),
        published: 2_000_000_000,
    }
}

fn library() -> Result<(tempfile::TempDir, Store, Feed, i64)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    store.subscribe("https://notes.example/feed")?;
    let feed = store.feeds()?.remove(0);
    store.ingest(&feed, "Notes", &[incoming("https://notes.example/old")])?;
    let article = store.library()?.articles[0].id;
    Ok((directory, store, feed, article))
}

#[test]
fn source_reads_keep_complete_feed_body_and_committed_annotation_changes()
-> Result<()> {
    let (directory, store, _feed, article) = library()?;
    let note = store.save_comment(article, None, "Earlier explanation")?;
    assert_eq!(store.note_detail(note)?.body, BODY.repeat(40));
    let writer = Store::open(&directory.path().join("library.sqlite3"))?;
    writer.save_comment(article, Some(note), "Revised café explanation")?;
    writer.set_tags(TagTarget::Comment(note), &["consensus".into()])?;
    writer.set_note_metadata(
        article,
        note,
        &NoteMetadata {
            kind: NoteKind::Question,
            pinned: true,
            ..Default::default()
        },
    )?;
    writer.create_study_card(article, note, "Who?", "The owner")?;

    let detail = store.note_detail(note)?;
    let exported = store.note_export(note)?;
    assert_eq!(detail.body, BODY.repeat(40));
    assert!(detail.source_available);
    assert_eq!(detail.source_revision, 0);
    assert_eq!(detail.url, "https://notes.example/old");
    assert_eq!(exported.url, detail.url);
    assert_eq!(exported.note.comment.body, "Revised café explanation");
    assert_eq!(exported.note.tags, ["consensus"]);
    assert_eq!(exported.note.metadata.kind, NoteKind::Question);
    assert!(exported.note.metadata.pinned);
    assert_eq!(exported.note.cards[0].answer, "The owner");
    Ok(())
}

#[test]
fn full_note_context_checks_current_url_and_retains_capture_revision()
-> Result<()> {
    let (_directory, mut store, feed, article) = library()?;
    let target = ArticleTarget {
        article,
        url: "https://notes.example/old".into(),
    };
    let mut document = ExtractedDocument {
        final_url: "https://notes.example/full".into(),
        body: "Full original quotation".into(),
    };
    assert!(store.cache_document(&target, &document)?);
    let note = store.save_passage(
        article,
        &Passage {
            quote: document.body.clone(),
            anchor: 0,
            source: ContentSource::Full,
            revision: 1,
        },
        "Private explanation",
    )?;
    document.body = "Full revised quotation".into();
    assert!(store.cache_document(&target, &document)?);
    let detail = store.note_detail(note)?;
    assert_eq!(detail.body, document.body);
    assert!(detail.source_available);
    assert_eq!(detail.source_revision, 2);
    assert_eq!(detail.note.comment.source_revision, 1);

    store.ingest(&feed, "Notes", &[incoming("https://notes.example/new")])?;
    let unavailable = store.note_detail(note)?;
    assert_eq!(unavailable.url, "https://notes.example/new");
    assert!(unavailable.body.is_empty());
    assert!(!unavailable.source_available);
    assert_eq!(unavailable.source_revision, 0);
    assert_eq!(unavailable.note.comment.quote, "Full original quotation");
    assert_eq!(store.note_export(note)?.url, unavailable.url);
    Ok(())
}

#[test]
fn note_source_reads_reject_trash_and_missing_notes_then_recover() -> Result<()>
{
    let (_directory, store, _feed, article) = library()?;
    let note = store.save_comment(article, None, "Visible explanation")?;
    store.note_detail(note)?;
    store.note_export(note)?;
    store.set_deleted(article, true)?;
    for id in [note, i64::MAX] {
        for error in [store.note_detail(id).err(), store.note_export(id).err()]
        {
            let error =
                error.context("Missing or trashed note was readable")?;
            assert_eq!(
                error.to_string(),
                "Note is missing or its article is in Trash"
            );
            assert!(matches!(
                error.downcast_ref::<rusqlite::Error>(),
                Some(rusqlite::Error::QueryReturnedNoRows)
            ));
        }
    }
    store.set_deleted(article, false)?;
    assert_eq!(store.note_detail(note)?.note.comment.id, note);
    assert_eq!(store.note_export(note)?.note.comment.id, note);
    store.delete_comment(article, note)?;
    assert!(store.note_export(note).is_err());
    assert!(store.undo()?);
    assert_eq!(store.note_export(note)?.note.comment.id, note);
    Ok(())
}
