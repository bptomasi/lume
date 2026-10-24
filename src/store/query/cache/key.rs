//! Debug SQL is not an identity: SQLite truncates expanded strings at NULs.
use super::super::{Criteria, filter_values};
use crate::model::ArticleQuery;
use anyhow::{Result, bail};
use rusqlite::types::{ToSqlOutput, Value};

#[derive(PartialEq)]
pub(in crate::store::query) struct Key {
    predicate: String,
    values: Vec<Value>,
    ranked: bool,
}

impl Key {
    pub(in crate::store::query) fn new(
        criteria: &Criteria,
        query: &ArticleQuery,
        predicate: String,
    ) -> Result<Self> {
        // Share the bind owner's named values, preserving SQLite types and all
        // text bytes. Pagination is deliberately excluded from membership.
        let mut values = filter_values(criteria, query)
            .into_iter()
            .map(|(_, value)| match value.to_sql()? {
                ToSqlOutput::Borrowed(value) => Ok(value.into()),
                ToSqlOutput::Owned(value) => Ok(value),
                _ => bail!("Unsupported page cache parameter"),
            })
            .collect::<Result<Vec<_>>>()?;
        values.extend(criteria.expression.values.iter().cloned());
        Ok(Self {
            predicate,
            values,
            ranked: criteria.ranking.is_some(),
        })
    }

    pub(in crate::store::query) fn predicate(&self) -> &str {
        &self.predicate
    }

    pub(super) fn bytes(&self) -> usize {
        self.predicate.capacity()
            + self.values.capacity() * std::mem::size_of::<Value>()
            + self
                .values
                .iter()
                .map(|value| match value {
                    Value::Text(text) => text.capacity(),
                    Value::Blob(bytes) => bytes.capacity(),
                    _ => 0,
                })
                .sum::<usize>()
    }
}
