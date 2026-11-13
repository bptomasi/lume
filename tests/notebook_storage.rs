//! Real SQLite scenarios qualify note-level search, provenance and recovery.

mod common;

use anyhow::{Context, Result};
use lume::{
    model::{
        ArticleQuery, ContentSource, DraftTarget, NoteDraft, NoteKind,
        NoteMetadata, Passage, TagTarget, View,
    },
    store::Store,
};

fn library() -> Result<(tempfile::TempDir, Store, i64)> {
    let (directory, store, _feed) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    Ok((directory, store, article))
}

fn article_query(text: &str) -> ArticleQuery {
    ArticleQuery {
        view: View::All,
        text: text.into(),
        offset: 0,
        limit: 100,
        retained: None,
    }
}

#[test]
fn note_predicates_cannot_be_satisfied_by_different_notes_in_one_article()
-> Result<()> {
    let (_directory, store, article) = library()?;
    let first =
        store.save_comment(article, None, "Consensus and leader election")?;
    let second =
        store.save_comment(article, None, "A question about consensus")?;
    store.set_tags(TagTarget::Comment(first), &["consensus".into()])?;
    store.set_note_metadata(
        article,
        second,
        &NoteMetadata {
            kind: NoteKind::Question,
            ..NoteMetadata::default()
        },
    )?;

    assert_eq!(
        store
            .search_notes("tag:consensus type:question", None, 0, 20)?
            .total,
        0
    );
    store.set_tags(TagTarget::Comment(second), &["consensus".into()])?;
    let page = store.search_notes(
        "tag:consensus type:question status:open",
        None,
        0,
        20,
    )?;

    assert_eq!(page.total, 1);
    assert_eq!(page.notes[0].id, second);
    assert_eq!(page.notes[0].metadata.kind, NoteKind::Question);
    assert_eq!(page.notes[0].tags, ["consensus"]);
    Ok(())
}

#[test]
fn independent_note_fts_preserves_article_search_and_handles_unicode_phrases()
-> Result<()> {
    let (_directory, store, article) = library()?;
    let id = store.save_comment(article, None, "A café and leader election")?;

    assert_eq!(store.search_notes("cafe", None, 0, 20)?.notes[0].id, id);
    assert_eq!(
        store
            .search_notes("\"leader election\"", None, 0, 20)?
            .total,
        1
    );
    assert_eq!(
        store
            .search_notes("leader AND NOT election", None, 0, 20)?
            .total,
        0
    );
    assert_eq!(store.query_articles(&article_query("notes:cafe"))?.total, 1);
    assert_eq!(store.query_articles(&article_query("cafe"))?.total, 1);

    store.save_comment(article, Some(id), "A different explanation")?;

    assert_eq!(store.search_notes("cafe", None, 0, 20)?.total, 0);
    assert_eq!(store.query_articles(&article_query("notes:cafe"))?.total, 0);
    assert_eq!(store.query_articles(&article_query("different"))?.total, 1);
    Ok(())
}

#[test]
fn paged_previews_and_metadata_filters_have_stable_bounded_results()
-> Result<()> {
    let (_directory, store, article) = library()?;
    for index in 0..105 {
        store.save_comment(article, None, &format!("Explanation {index}"))?;
    }
    let first = store.comments(article)?[0].id;
    store.set_note_metadata(
        article,
        first,
        &NoteMetadata {
            pinned: true,
            resolved: true,
            ..NoteMetadata::default()
        },
    )?;

    let page = store.search_notes("", None, 0, usize::MAX)?;

    assert_eq!(page.total, 105);
    assert_eq!(page.notes.len(), 100);
    assert_eq!(page.notes[0].id, first);
    assert_eq!(store.search_notes("", None, 100, 20)?.notes.len(), 5);
    assert_eq!(
        store
            .search_notes("is:pinned status:resolved", Some(article), 0, 20)?
            .total,
        1
    );
    store.set_deleted(article, true)?;
    assert_eq!(store.search_notes("", None, 0, 20)?.total, 0);
    assert!(store.note_summary(first).is_err());
    Ok(())
}

#[test]
fn note_identity_is_never_reused_after_deletion_or_restart() -> Result<()> {
    let (directory, store, article) = library()?;
    let old = store.save_comment(article, None, "Earlier note")?;
    store.delete_comment(article, old)?;
    drop(store);
    let store = Store::open(&directory.path().join("library.sqlite3"))?;

    let new = store.save_comment(article, None, "A different note")?;

    assert!(new > old);
    assert_eq!(store.note_versions(article, new)?.len(), 1);
    assert!(store.undo()?);
    assert!(store.undo()?);
    assert_eq!(store.comments(article)?[0].id, old);
    assert_eq!(store.comments(article)?[0].body, "Earlier note");
    Ok(())
}

#[test]
fn wikilinks_keep_backlinks_and_deletion_undo_preserves_both_directions()
-> Result<()> {
    let (_directory, store, article) = library()?;
    let target = store.save_comment(article, None, "A consensus definition")?;
    let source = store.save_comment(
        article,
        None,
        &format!(
            "Compare [[Note-{target}|Consensus]]\n\n```\n[[Note-999999]]\n```"
        ),
    )?;
    let expected = vec![(source, target)];
    assert_eq!(
        store.annotation(article, source)?.metadata.references,
        expected
    );
    let target_metadata = store.annotation(article, target)?.metadata;
    assert_eq!(target_metadata.references, expected);
    store.set_note_metadata(
        article,
        target,
        &NoteMetadata {
            kind: NoteKind::Definition,
            pinned: true,
            ..NoteMetadata::default()
        },
    )?;

    store.delete_comment(article, target)?;
    assert!(
        store
            .annotation(article, source)?
            .metadata
            .references
            .is_empty()
    );
    assert!(store.undo()?);

    let restored = store.annotation(article, target)?;
    assert_eq!(restored.metadata.kind, NoteKind::Definition);
    assert!(restored.metadata.pinned);
    assert_eq!(restored.metadata.references, expected);
    Ok(())
}

#[test]
fn invalid_links_fail_atomically_without_extra_versions_or_undo() -> Result<()>
{
    let (_directory, store, article) = library()?;
    let id = store.save_comment(article, None, "Original explanation")?;
    store.clear_history()?;

    assert!(
        store
            .save_comment(article, Some(id), "[[Note-999999]]")
            .is_err()
    );
    assert!(
        store
            .save_comment(article, Some(id), &format!("[[Note-{id}]]"))
            .is_err()
    );

    assert_eq!(store.comments(article)?[0].body, "Original explanation");
    assert_eq!(store.note_versions(article, id)?.len(), 1);
    assert!(!store.undo()?);
    Ok(())
}

#[test]
fn recovery_checkpoint_is_consumed_in_the_same_commit_as_a_new_note()
-> Result<()> {
    let (directory, store, article) = library()?;
    let draft = NoteDraft {
        initial_kind: None,
        target: DraftTarget::Note {
            article,
            note: None,
        },
        body: "Uncommitted user explanation".into(),
        baseline: String::new(),
    };
    store.save_note_draft(&draft)?;
    let reopened = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(reopened.note_drafts()?[0].body, draft.body);
    assert!(reopened.comments(article)?.is_empty());

    let id = reopened.save_comment(article, None, &draft.body)?;

    assert!(store.note_drafts()?.is_empty());
    assert_eq!(store.comments(article)?[0].id, id);
    Ok(())
}

#[test]
fn checkpoint_conflicts_preserve_the_earlier_source_evidence() -> Result<()> {
    let (_directory, store, article) = library()?;
    let mut draft = NoteDraft {
        initial_kind: None,
        target: DraftTarget::Passage {
            article,
            passage: Passage {
                quote: "Earlier quotation".into(),
                anchor: 0,
                source: ContentSource::Feed,
                revision: 0,
            },
        },
        body: "A recovered explanation".into(),
        baseline: String::new(),
    };
    store.save_note_draft(&draft)?;
    if let DraftTarget::Passage { passage, .. } = &mut draft.target {
        passage.quote = "Changed source".into();
    }

    assert!(store.save_note_draft(&draft).is_err());

    let saved = store.note_drafts()?;
    let DraftTarget::Passage { passage, .. } = &saved[0].target else {
        anyhow::bail!("Recovery target changed kind");
    };
    assert_eq!(passage.quote, "Earlier quotation");
    Ok(())
}

#[test]
fn restored_versions_require_the_current_preview_and_remain_reversible()
-> Result<()> {
    let (_directory, store, article) = library()?;
    let id = store.save_comment(article, None, "First explanation")?;
    let version = store.note_versions(article, id)?[0].id;
    store.save_comment(article, Some(id), "Second explanation")?;
    let current = store.comments(article)?[0].clone();
    store.clear_history()?;

    store.restore_note_version(article, &current, version)?;

    assert_eq!(store.comments(article)?[0].body, "First explanation");
    assert!(
        store
            .restore_note_version(article, &current, version)
            .is_err()
    );
    assert!(store.undo()?);
    assert_eq!(store.comments(article)?[0].body, "Second explanation");
    assert!(!store.undo()?);
    Ok(())
}

#[test]
fn saved_collections_and_history_use_the_live_query_contract() -> Result<()> {
    let (_directory, store, _article) = library()?;
    store.save_note_collection("Questions", "type:question status:open")?;
    assert!(
        store
            .save_note_collection("Broken", "invalid:value")
            .is_err()
    );
    store.save_note_collection("Questions", "type:question")?;
    for index in 0..35 {
        store.remember_note_search(&format!("body:term{index}"))?;
    }
    store.remember_note_search("body:term10")?;

    let collections = store.note_collections()?;
    let history = store.note_search_history()?;

    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].query, "type:question");
    assert_eq!(history.len(), 30);
    assert_eq!(history[0], "body:term10");
    assert_eq!(history[1], "body:term34");
    Ok(())
}

#[test]
fn note_versions_are_bounded_and_keep_the_latest_committed_text() -> Result<()>
{
    let (_directory, store, article) = library()?;
    let id = store.save_comment(article, None, "Version zero")?;
    for version in 1..110 {
        store.save_comment(article, Some(id), &format!("Version {version}"))?;
    }

    let versions = store.note_versions(article, id)?;

    assert_eq!(versions.len(), 100);
    assert_eq!(versions[0].comment.body, "Version 109");
    for _ in 0..3 {
        store.delete_comment(article, id)?;
        store.undo()?;
        assert_eq!(store.note_versions(article, id)?.len(), 100);
    }
    assert_eq!(
        versions.last().context("No versions")?.comment.body,
        "Version 10"
    );
    Ok(())
}

#[test]
fn a_retired_reference_never_prevents_editing_its_surviving_source()
-> Result<()> {
    let (_directory, store, article) = library()?;
    let target = store.save_comment(article, None, "Definition")?;
    let body = format!("Evidence in [[Note-{target}]]");
    let source = store.save_comment(article, None, &body)?;
    store.delete_comment(article, target)?;

    store.save_comment(
        article,
        Some(source),
        &format!("{body} with more evidence"),
    )?;

    assert!(
        store
            .annotation(article, source)?
            .metadata
            .references
            .is_empty()
    );
    assert!(store.comments(article)?[0].body.contains("more evidence"));
    store.undo()?;
    store.undo()?;
    assert_eq!(
        store.annotation(article, source)?.metadata.references,
        vec![(source, target)]
    );
    Ok(())
}

#[test]
fn a_passage_template_commits_its_kind_and_source_in_the_same_annotation()
-> Result<()> {
    let (_directory, store, article) = library()?;
    let draft = NoteDraft {
        target: DraftTarget::Passage {
            article,
            passage: Passage {
                quote: "Keep".into(),
                anchor: 0,
                source: ContentSource::Feed,
                revision: 0,
            },
        },
        body: "Why does this preserve ordering?".into(),
        baseline: String::new(),
        initial_kind: Some(NoteKind::Question),
    };

    let target = store.write_note_draft(&draft)?;
    let DraftTarget::Note { note: Some(id), .. } = target else {
        anyhow::bail!("A passage must become a persisted note");
    };
    let saved = store.annotation(article, id)?;
    assert_eq!(saved.metadata.kind, NoteKind::Question);
    assert_eq!(saved.comment.quote, "Keep");
    assert_eq!(saved.comment.source, ContentSource::Feed);
    assert_eq!(store.note_draft_count()?, 0);
    store.undo()?;
    assert!(store.comments(article)?.is_empty());
    Ok(())
}

#[test]
fn undoing_note_tags_invalidates_only_their_owning_annotation_cache()
-> Result<()> {
    let (_directory, store, article) = library()?;
    let note = store.save_comment(article, None, "Keep the source cached")?;
    store.clear_history()?;
    store.set_tags(TagTarget::Comment(note), &["consensus".into()])?;

    let effect = store.undo_effect()?;

    assert_eq!(effect, lume::model::UndoEffect::Notes(article));
    assert!(store.tags(TagTarget::Comment(note))?.is_empty());
    assert_eq!(store.comments(article)?[0].body, "Keep the source cached");
    assert!(!store.undo()?);
    Ok(())
}

#[test]
fn markdown_indentation_and_saved_baselines_keep_the_exact_editor_text()
-> Result<()> {
    let (_directory, store, article) = library()?;
    let original = "    [[Note-999999]]\n    code example\n\n";
    let id = store.save_comment(article, None, original)?;
    assert_eq!(store.comments(article)?[0].body, original);
    assert!(
        store
            .annotation(article, id)?
            .metadata
            .references
            .is_empty()
    );
    let edited = "\t[[Note-999999]]\n\tmore code\n";

    store.save_comment_checked(article, Some(id), edited, Some(original))?;

    assert_eq!(store.comments(article)?[0].body, edited);
    assert!(
        store
            .annotation(article, id)?
            .metadata
            .references
            .is_empty()
    );
    let rendered = lume::reader::render(edited, 72);
    assert!(rendered.code.iter().any(|line| *line));
    store.undo()?;
    assert_eq!(store.comments(article)?[0].body, original);
    Ok(())
}
