//! Ranked identities are paginated before article metadata is hydrated.
//!
//! Pages and identity-based jumps share one total order, including date/id
//! tie breakers. Count queries deliberately omit scoring work entirely.

use anyhow::Result;
use rusqlite::{Statement, ToSql};

use crate::search::ranking::Ranking;

const SCORES: &str = include_str!("../sql/search/select_relevance.sql");
const ORDER: &str = include_str!("../sql/search/select_relevance_order.sql");

fn candidates(predicate: &str) -> String {
    format!(
        include_str!("../sql/search/select_relevant_candidates.sql"),
        predicate = predicate,
        scores = SCORES,
    )
}

pub(super) fn page_sql(predicate: &str) -> String {
    format!(
        include_str!("../sql/search/select_relevant_page.sql"),
        candidates = candidates(predicate),
        order = ORDER,
        metadata = include_str!("../sql/select_article_metadata.sql"),
    )
}

pub(super) fn position_sql(predicate: &str) -> String {
    format!(
        include_str!("../sql/search/select_relevant_position.sql"),
        candidates = candidates(predicate),
        order = ORDER,
    )
}

pub(super) fn bind(
    statement: &mut Statement<'_>,
    rank: &Ranking,
) -> Result<()> {
    let values: &[(&str, &dyn ToSql)] = &[
        (":rank_text", &rank.text),
        (":rank_title", &rank.title),
        (":rank_phrase", &rank.phrase),
        (":rank_prefix", &rank.prefix),
    ];
    for (name, value) in values {
        if let Some(index) = statement.parameter_index(name)? {
            statement.raw_bind_parameter(index, value)?;
        }
    }
    Ok(())
}
