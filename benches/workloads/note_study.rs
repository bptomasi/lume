//! Correctness-checked recovery, metadata, references, review and repair cycles.

use anyhow::{Context, Result, ensure};
use lume::{
    model::{
        AnnotationSnapshot, ContentSource, DraftTarget, NoteChange, NoteDraft,
        Passage, TagTarget,
    },
    store::Store,
};

struct Scenario {
    before: AnnotationSnapshot,
    edited: String,
    passage: Passage,
    version: i64,
}

pub fn run(
    store: &Store,
    note: i64,
    iterations: usize,
    stress: bool,
) -> Result<()> {
    let mut scenario = scenario(store, note, stress)?;
    let article = scenario.before.article;
    super::measure(
        "checkpoint + references + metadata + review + repair/version + undo",
        iterations,
        || {
            scenario.version = store.note_versions(article, note)?[0].id;
            change(store, &scenario)?;
            verify_changed(store, &scenario)?;
            for _ in 0..7 {
                ensure!(store.undo()?, "Study change lost its undo entry");
            }
            let restored = store.annotation(article, note)?;
            ensure!(
                restored.comment == scenario.before.comment
                    && restored.cards == scenario.before.cards
                    && restored.metadata == scenario.before.metadata
                    && restored.tags == scenario.before.tags
                    && restored.review == scenario.before.review,
                "Study cycle failed to restore its exact independent baseline"
            );
            Ok(())
        },
    )
}

fn scenario(store: &Store, note: i64, stress: bool) -> Result<Scenario> {
    let article = store.note_summary(note)?.article;
    let answer = "The transaction owner preserves ordering across retries. "
        .repeat(if stress { 60 } else { 8 });
    for index in 0..if stress { 16 } else { 4 } {
        store.create_study_card(
            article,
            note,
            &format!("Which owner preserves ordering in scenario {index}?"),
            &answer,
        )?;
    }
    let before = store.annotation(article, note)?;
    let body = store.body(article)?;
    let quote = "Keep the protocol precise and work bounded.";
    let scenario = Scenario {
        edited: format!(
            "{}\n\nCompare [[Note-{}]].",
            before.comment.body,
            note - 10
        ),
        before,
        passage: Passage {
            quote: quote.into(),
            anchor: body.find(quote).context("Missing source passage")?,
            source: ContentSource::Feed,
            revision: 0,
        },
        version: 0,
    };
    Ok(scenario)
}

fn change(store: &Store, scenario: &Scenario) -> Result<()> {
    let before = &scenario.before;
    let (article, note) = (before.article, before.comment.id);
    store.write_note_draft(&NoteDraft {
        target: DraftTarget::Note {
            article,
            note: Some(note),
        },
        body: scenario.edited.clone(),
        baseline: before.comment.body.clone(),
        initial_kind: None,
    })?;
    store.change_note_metadata(article, note, NoteChange::TogglePin)?;
    store.set_tags(
        TagTarget::Comment(note),
        &["consensus".into(), "review".into()],
    )?;
    let review = store.annotation(article, note)?;
    ensure!(
        review.metadata.references == vec![(note, note - 10)],
        "Reference graph changed"
    );
    store.review_captured_annotation(&review, 3, 100)?;
    store.review_study_card(&review.cards[0], 7, 100)?;
    let current = store.annotation(article, note)?;
    store.reanchor_note(article, &current.comment, &scenario.passage)?;
    let repaired = store.annotation(article, note)?;
    ensure!(
        repaired.comment.quote == scenario.passage.quote
            && repaired.comment.anchor == scenario.passage.anchor,
        "Anchor repair changed source evidence"
    );
    store.restore_note_version(article, &repaired.comment, scenario.version)
}

fn verify_changed(store: &Store, scenario: &Scenario) -> Result<()> {
    let before = &scenario.before;
    let current = store.annotation(before.article, before.comment.id)?;
    ensure!(
        current.comment.body == before.comment.body
            && current.comment.quote == before.comment.quote,
        "Historical restoration changed its previewed text"
    );
    ensure!(
        current.metadata.pinned
            && current.cards.len() == before.cards.len()
            && current.cards[0].reviews == 1
            && current.cards[0].due_at == 604_900
            && current.review.as_ref().is_some_and(|review| review.reviews
                == 1
                && review.due_at == 259_300),
        "Restoration lost independent study metadata"
    );
    ensure!(
        current.tags == vec!["consensus", "review"]
            && store.note_draft_count()? == 0,
        "Committed study text lost tags or retained a consumed checkpoint"
    );
    Ok(())
}
