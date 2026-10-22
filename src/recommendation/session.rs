//! A bounded greedy plan balances relevance, source variety and available time.
//!
//! This deliberately does not claim optimal knapsack selection: familiar
//! recommendation ordering is adjusted for repeated sources and interests.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};

use crate::model::{ReadingSession, SessionItem};

/// Plan at most eight articles from 512 metadata candidates within a total budget.
///
/// # Errors
/// Returns a budget outside 1–240 minutes or invalid captured metadata.
pub fn plan(
    mut candidates: Vec<SessionItem>,
    budget: u16,
) -> Result<ReadingSession> {
    ensure!(
        (1..=240).contains(&budget),
        "Session budget is 1–240 minutes"
    );
    candidates.truncate(512);
    candidates.retain(|item| item.minutes > 0 && item.score > 0);
    let mut remaining = budget;
    let mut sources = BTreeMap::<i64, i64>::new();
    let mut topics = BTreeMap::<String, i64>::new();
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();
    while items.len() < 8 {
        let best = best_candidate(&candidates, remaining, &sources, &topics);
        let Some(best) = best else {
            break;
        };
        let item = candidates.swap_remove(best);
        if !item.canonical.is_empty() && !seen.insert(item.canonical.clone()) {
            continue;
        }
        remaining -= item.minutes;
        *sources.entry(item.feed).or_default() += 1;
        *topics.entry(item.topic.clone()).or_default() += 1;
        items.push(item);
    }
    let session = ReadingSession {
        budget,
        items,
        position: 0,
    };
    session.validate()?;
    Ok(session)
}

fn best_candidate(
    candidates: &[SessionItem],
    remaining: u16,
    sources: &BTreeMap<i64, i64>,
    topics: &BTreeMap<String, i64>,
) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .filter(|(_, item)| item.minutes <= remaining)
        .max_by_key(|(_, item)| {
            let divisor = 4
                + sources.get(&item.feed).copied().unwrap_or(0) * 4
                + topics.get(&item.topic).copied().unwrap_or(0) * 2;
            (
                item.score.saturating_mul(1000) / divisor,
                -i64::from(item.minutes),
                item.article,
            )
        })
        .map(|(index, _)| index)
}
