use std::collections::{BTreeMap, BTreeSet};

/// A body-free scored candidate; at most 512 reach diversity selection.
#[derive(Clone, Debug)]
pub struct Candidate {
    /// Article identity.
    pub article: i64,
    /// Subscription identity for diversity.
    pub feed: i64,
    /// Canonical URL key; an empty key remains unique by article identity.
    pub canonical: String,
    /// Positive evidence score, independent of publication age.
    pub score: i64,
    /// Highest-priority matching interest, used to balance the shortlist.
    pub topic: String,
    /// Deterministic tie-break, retaining useful older articles.
    pub published: i64,
}

/// Select up to 100 entries with diminishing returns per source and interest.
/// Work is bounded by a 512-candidate pool; duplicates retain their best entry.
pub fn select(mut candidates: Vec<Candidate>) -> Vec<i64> {
    candidates.truncate(512);
    let mut sources = BTreeMap::<i64, i64>::new();
    let mut topics = BTreeMap::<String, i64>::new();
    let mut seen = BTreeSet::new();
    let mut selected = Vec::new();
    while selected.len() < 100 && !candidates.is_empty() {
        let best = candidates
            .iter()
            .enumerate()
            .max_by_key(|(_, item)| {
                let divisor = 4
                    + sources.get(&item.feed).copied().unwrap_or(0) * 2
                    + topics.get(&item.topic).copied().unwrap_or(0);
                (
                    item.score.saturating_mul(1000) / divisor,
                    item.published,
                    item.article,
                )
            })
            .map(|(index, _)| index)
            .unwrap_or(0);
        let item = candidates.swap_remove(best);
        if !item.canonical.is_empty() && !seen.insert(item.canonical) {
            continue;
        }
        *sources.entry(item.feed).or_default() += 1;
        *topics.entry(item.topic).or_default() += 1;
        selected.push(item.article);
    }
    selected
}
