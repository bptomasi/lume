//! Per-note exports preserve local edits, stable links and review ownership.

mod common;

use anyhow::{Context, Result};
use lume::{
    exchange,
    model::{NoteKind, NoteMetadata},
    store::Store,
};

#[test]
fn note_files_keep_stable_wikilinks_and_never_overwrite_local_changes()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    let target = store.save_composed_note(
        article,
        "A definition",
        NoteKind::Definition,
    )?;
    let source = store.save_composed_note(
        article,
        &format!("Evidence in [[Note-{target}]]"),
        NoteKind::Synthesis,
    )?;
    let output = directory.path().join("obsidian");

    assert_eq!(exchange::notes::notebook(&store, &output)?.written, 2);
    let target_file = output.join(format!("Note-{target}.md"));
    let source_file = output.join(format!("Note-{source}.md"));
    let bytes = std::fs::read_to_string(&target_file)?;
    assert!(bytes.contains("type: definition"));
    assert!(bytes.contains(&format!("[[Note-{source}]]")));
    assert!(bytes.contains("source: \"https://"));
    assert_eq!(exchange::notes::notebook(&store, &output)?.unchanged, 2);

    std::fs::write(&target_file, "My local annotation\n")?;
    store.save_comment(article, Some(target), "A revised definition")?;
    let report = exchange::notes::notebook(&store, &output)?;
    assert_eq!(report.conflicts, vec![target_file.clone()]);
    assert_eq!(
        std::fs::read_to_string(&target_file)?,
        "My local annotation\n"
    );
    store.delete_comment(article, source)?;
    exchange::notes::notebook(&store, &output)?;
    assert!(source_file.exists());
    Ok(())
}

#[test]
fn postponement_keeps_review_counts_and_undo_restores_exact_schedules()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    let note = store.save_comment(article, None, "Recall the mechanism")?;
    let card = store.create_study_card(article, note, "Who?", "The owner")?;
    let before = store.annotation(article, note)?;

    store.postpone_annotation(&before, Some(card), 3, 100)?;
    let after = store.annotation(article, note)?;
    assert_eq!(after.cards[0].reviews, 0);
    assert_eq!(after.cards[0].due_at, 259_300);
    assert!(
        store
            .postpone_annotation(&before, Some(card), 1, 100)
            .is_err()
    );
    assert!(store.undo()?);
    assert_eq!(store.annotation(article, note)?.cards, before.cards);
    store.postpone_annotation(&before, None, 1, 100)?;
    let schedule = store
        .annotation(article, note)?
        .review
        .context("Schedule missing")?;
    assert_eq!(schedule.reviews, 0);
    assert_eq!(schedule.due_at, 86_500);
    let reopened = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(reopened.annotation(article, note)?.review, Some(schedule));
    Ok(())
}

#[test]
fn review_filters_apply_before_the_due_card_limit() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    for index in 0..205 {
        let note = store.save_comment(article, None, "Unrelated evidence")?;
        store.create_study_card(
            article,
            note,
            &format!("Question {index}"),
            "Own answer",
        )?;
    }
    let note = store.save_comment(article, None, "Consensus mechanism")?;
    store.set_note_metadata(
        article,
        note,
        &NoteMetadata {
            kind: NoteKind::Definition,
            ..Default::default()
        },
    )?;
    store.create_study_card(
        article,
        note,
        "What defines a quorum?",
        "Own explanation",
    )?;

    let cards =
        store.filtered_study_cards("type:definition body:consensus", 100)?;
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].comment, note);
    assert_eq!(store.filtered_study_cards("", 100)?.len(), 200);
    Ok(())
}
