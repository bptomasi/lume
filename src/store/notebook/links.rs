//! Pulldown-cmark owns wikilink parsing; this owner resolves stable note IDs.
//!
//! Only actual Markdown links participate: examples inside code fences remain
//! examples. References are bounded and checked before the transaction commits.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use pulldown_cmark::{Event, LinkType, Options, Parser, Tag};
use rusqlite::Transaction;

pub(crate) fn replace_references(
    transaction: &Transaction<'_>,
    note: i64,
    body: &str,
) -> Result<()> {
    let mut targets = Vec::new();
    for target in references(body)? {
        if let Some(target) = live_target(transaction, note, target)? {
            targets.push(target);
        }
    }
    transaction
        .execute("DELETE FROM note_references WHERE source_id = ?1", [note])?;
    for target in targets {
        transaction.execute(
            "INSERT INTO note_references (source_id, target_id) VALUES (?1, ?2)",
            (note, target),
        )?;
    }
    Ok(())
}

fn live_target(
    transaction: &Transaction<'_>,
    note: i64,
    target: i64,
) -> Result<Option<i64>> {
    ensure!(target != note, "A note cannot reference itself");
    let live: Option<i64> = transaction
        .query_row(
            include_str!("../sql/notebook/select_note_reference_targets.sql"),
            [target],
            |row| row.get(0),
        )
        .context("Referenced note ID has never existed")?;
    // Retired identities remain meaningful in text. Editing their source must
    // remain possible; restoring the target can restore its captured edges.
    if live.is_some() {
        let incoming: usize = transaction.query_row(
            r#"
            SELECT count(*)
            FROM note_references
            WHERE target_id = ?1 AND source_id <> ?2
            "#,
            (target, note),
            |row| row.get(0),
        )?;
        ensure!(incoming < 128, "A note can have at most 128 backlinks");
    }
    Ok(live)
}

fn references(body: &str) -> Result<BTreeSet<i64>> {
    let mut targets = BTreeSet::new();
    for event in Parser::new_ext(body, Options::ENABLE_WIKILINKS) {
        if let Event::Start(Tag::Link {
            link_type: LinkType::WikiLink { .. },
            dest_url,
            ..
        }) = event
            && let Some(id) = dest_url.strip_prefix("Note-")
        {
            let id = id
                .parse::<i64>()
                .context("Use [[Note-ID|label]] for note references")?;
            ensure!(id > 0, "Note IDs must be positive");
            targets.insert(id);
            ensure!(
                targets.len() <= 128,
                "A note can reference at most 128 notes"
            );
        }
    }
    Ok(targets)
}

pub(crate) fn restore_references(
    transaction: &Transaction<'_>,
    note: i64,
    references: &[(i64, i64)],
) -> Result<()> {
    ensure!(references.len() <= 256, "Invalid saved note references");
    transaction.execute(
        "DELETE FROM note_references WHERE source_id = ?1 OR target_id = ?1",
        [note],
    )?;
    for (source, target) in references {
        ensure!(
            *source == note || *target == note,
            "Reference has another owner"
        );
        transaction.execute(
            "INSERT OR IGNORE INTO note_references (source_id, target_id) VALUES (?1, ?2)",
            (source, target),
        )?;
    }
    Ok(())
}
