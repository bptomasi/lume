//! Annotation children survive undo; review schedules are explicit and durable.

mod common;

use anyhow::{Context, Result};
use lume::{exchange, model::TagTarget, store::Store};

fn annotated() -> Result<(tempfile::TempDir, Store, i64, i64)> {
    let (directory, store, _feed) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    store.save_highlight(article, "Keep", 0, "Why does one writer help?")?;
    let comment = store.comments(article)?[0].id;
    Ok((directory, store, article, comment))
}

#[test]
fn tags_links_cards_and_review_dates_survive_annotation_deletion_undo()
-> Result<()> {
    let (directory, store, article, comment) = annotated()?;
    store.set_tags(TagTarget::Comment(comment), &["consensus".into()])?;
    store.link_annotation(article, comment, article)?;
    let card = store.create_study_card(
        article,
        comment,
        "Who owns the write?",
        "The transaction owner.",
    )?;
    store.review_annotation(article, comment, 3, 100)?;
    let captured = store
        .annotation(article, comment)?
        .cards
        .into_iter()
        .find(|item| item.id == card)
        .context("Card missing")?;
    store.review_study_card(&captured, 7, 100)?;
    assert!(store.review_study_card(&captured, 7, 100).is_err());
    store.clear_history()?;

    store.delete_comment(article, comment)?;
    assert!(store.comments(article)?.is_empty());
    let reopened = Store::open(&directory.path().join("library.sqlite3"))?;
    assert!(reopened.undo()?);

    let restored = reopened.annotation(article, comment)?;
    assert_eq!(restored.tags, ["consensus"]);
    assert_eq!(restored.links, [article]);
    assert_eq!(restored.comment.quote, "Keep");
    assert_eq!(restored.review.context("Review lost")?.due_at, 259_300);
    assert_eq!(restored.cards[0].answer, "The transaction owner.");
    assert_eq!(restored.cards[0].due_at, 604_900);
    Ok(())
}

#[test]
fn review_due_boundaries_invalid_intervals_and_trash_are_explicit() -> Result<()>
{
    let (_directory, store, article, comment) = annotated()?;
    assert_eq!(store.due_annotations(100)?, [(article, comment)]);
    store.review_annotation(article, comment, 1, 100)?;
    assert!(store.due_annotations(86_499)?.is_empty());
    assert_eq!(store.due_annotations(86_500)?, [(article, comment)]);
    assert!(store.review_annotation(article, comment, 0, 100).is_err());
    assert!(
        store
            .create_study_card(article + 1000, comment, "Question?", "Answer")
            .is_err()
    );
    assert!(
        store
            .create_study_card(article, comment, "", "Answer")
            .is_err()
    );
    store.set_deleted(article, true)?;
    assert!(store.due_annotations(i64::MAX)?.is_empty());
    assert!(store.due_study_cards(i64::MAX)?.is_empty());
    Ok(())
}

#[test]
fn incremental_exports_are_stable_and_preserve_local_edits_and_unowned_files()
-> Result<()> {
    let (directory, store, article, comment) = annotated()?;
    let output = directory.path().join("vault");
    let first = exchange::incremental::notebook(&store, &output)?;
    assert_eq!(
        (first.written, first.unchanged, first.conflicts.len()),
        (1, 0, 0)
    );
    let path = output.join(format!("Lume-{article}.md"));
    let original = std::fs::read(&path)?;
    let second = exchange::incremental::notebook(&store, &output)?;
    assert_eq!((second.written, second.unchanged), (0, 1));
    store.save_comment(article, Some(comment), "A revised private note.")?;
    assert_eq!(exchange::incremental::notebook(&store, &output)?.written, 1);
    assert_ne!(std::fs::read(&path)?, original);

    std::fs::write(&path, "My own Obsidian edits.\n")?;
    store.save_comment(article, Some(comment), "Another revision.")?;
    let conflicted = exchange::incremental::notebook(&store, &output)?;
    assert_eq!(conflicted.conflicts.as_slice(), std::slice::from_ref(&path));
    assert_eq!(std::fs::read_to_string(&path)?, "My own Obsidian edits.\n");
    let unowned = directory.path().join("unowned");
    std::fs::create_dir(&unowned)?;
    std::fs::write(
        unowned.join(format!("Lume-{article}.md")),
        "Existing unrelated file",
    )?;
    assert_eq!(
        exchange::incremental::notebook(&store, &unowned)?
            .conflicts
            .len(),
        1
    );
    Ok(())
}

fn app_command(app: &mut lume::app::App, text: &str) -> Result<()> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(text)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}

#[test]
fn passage_review_rejects_a_note_changed_since_its_preview_opened() -> Result<()>
{
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (directory, store, article, comment) = annotated()?;
    let writer = Store::open(&directory.path().join("library.sqlite3"))?;
    let mut app = lume::app::App::new(store, false)?;
    app_command(&mut app, "review")?;
    writer.save_comment(article, Some(comment), "Revised explanation")?;

    let reviewed = app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(reviewed.is_err());
    let actual = writer.annotation(article, comment)?;
    assert_eq!(actual.comment.body, "Revised explanation");
    assert!(actual.review.is_none());
    Ok(())
}

#[test]
fn captured_passage_reviews_reject_concurrent_reviews_without_another_inverse()
-> Result<()> {
    let (_directory, store, article, comment) = annotated()?;
    let preview = store.annotation(article, comment)?;
    store.review_annotation(article, comment, 3, 100)?;
    store.clear_history()?;

    assert!(store.review_captured_annotation(&preview, 7, 100).is_err());

    let review = store
        .annotation(article, comment)?
        .review
        .context("Review disappeared")?;
    assert_eq!((review.interval_days, review.reviews), (3, 1));
    assert!(!store.undo()?);
    Ok(())
}

#[test]
fn captured_reviews_and_study_edits_require_an_active_article() -> Result<()> {
    let (_directory, store, article, comment) = annotated()?;
    store.create_study_card(article, comment, "Who writes?", "The owner.")?;
    let preview = store.annotation(article, comment)?;
    store.set_deleted(article, true)?;
    store.clear_history()?;

    assert!(store.review_captured_annotation(&preview, 7, 100).is_err());
    assert!(store.review_study_card(&preview.cards[0], 7, 100).is_err());
    assert!(store.review_annotation(article, comment, 7, 100).is_err());
    assert!(
        store
            .create_study_card(article, comment, "New?", "Yes.")
            .is_err()
    );
    assert!(store.link_annotation(article, comment, article).is_err());
    assert!(!store.undo()?);
    assert!(store.annotation(article, comment)?.review.is_none());
    Ok(())
}
