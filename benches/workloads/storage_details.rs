//! Diagnostic timing checks exact publisher/FTS byte amplification.
use anyhow::{Result, ensure};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let count = if stress { 13_000 } else { 2000 };
    let body =
        "Systems preserve independent sources and personal reading state. "
            .repeat(32);
    let publisher_bytes = count * body.len();
    let store = super::page_navigation::fixture_with_body(
        &directory.path().join("storage.sqlite3"),
        count,
        &body,
    )?;
    println!(
        "storage diagnostics · articles={count} · publisher bytes={}",
        publisher_bytes
    );
    let measured = lume::storage::details::inspect(&store)?;
    println!(
        "FTS duplication: {}",
        serde_json::to_string(&measured.duplication)?
    );
    super::measure(
        "source/table/FTS snapshot diagnostics",
        iterations.min(8),
        || verify(&lume::storage::details::inspect(&store)?, publisher_bytes),
    )
}

fn verify(
    details: &lume::storage::details::Details,
    publisher_bytes: usize,
) -> Result<()> {
    ensure!(
        details.duplication.publisher_body_bytes == publisher_bytes as u64,
        "Publisher byte drift"
    );
    ensure!(
        details.fts_body_bytes == publisher_bytes as u64,
        "FTS byte drift"
    );
    ensure!(
        details.duplication.fts_content_allocated_bytes > 0
            && details.duplication.fts_other_allocated_bytes > 0,
        "Missing physical FTS pages"
    );
    ensure!(
        details.text(lume::storage::Unit::MiB).contains("1.000×"),
        "Ratio disappeared"
    );
    Ok(())
}
