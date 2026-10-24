//! Bulk deletion reuses view predicates without counting or loading bodies.

use anyhow::Result;

use super::{Store, bind};
use crate::model::{ArticleQuery, View};

impl Store {
    pub(in crate::store) fn trash_matching(
        &self,
        view: &View,
    ) -> Result<Vec<i64>> {
        let query = ArticleQuery {
            // Emptying grouped articles includes every duplicate occurrence.
            view: if *view == View::Grouped {
                View::All
            } else {
                view.clone()
            },
            text: String::new(),
            offset: 0,
            limit: 1,
            retained: None,
        };
        let criteria = self.criteria(&query)?;
        let sql = include_str!("../sql/deletion/trash_matching.sql")
            .replace("{matching}", &criteria.predicate());
        let mut statement = self.connection.prepare_cached(&sql)?;
        bind(&mut statement, &criteria, &query)?;
        let mut rows = statement.raw_query();
        let mut articles = Vec::new();
        while let Some(row) = rows.next()? {
            articles.push(row.get(0)?);
        }
        Ok(articles)
    }
}
