//! One snapshot batches ordinary destinations and shares identical saved queries.
use super::Store;
use crate::model::View;
use anyhow::Result;
use std::collections::HashMap;

impl Store {
    /// Count destinations in order, sharing ordinary scans and saved filters.
    ///
    /// # Errors
    /// Returns invalid saved queries or SQLite errors without publishing a slice.
    pub fn view_counts(&self, views: &[View]) -> Result<Vec<usize>> {
        self.read_snapshot(|store| {
            let mut counts = store.navigation_totals(views)?;
            let mut saved = HashMap::new();
            views
                .iter()
                .map(|view| {
                    if let Some(count) = counts.get(view) {
                        return Ok(*count);
                    }
                    let count = if let View::Smart(id) = view {
                        let key =
                            serde_json::to_string(&store.smart_filter(*id)?)?;
                        if let Some(count) = saved.get(&key) {
                            *count
                        } else {
                            let count = store.view_count(view)?;
                            saved.insert(key, count);
                            count
                        }
                    } else if matches!(
                        view,
                        View::Feed(_) | View::Server(_) | View::Collection(_)
                    ) {
                        0
                    } else {
                        store.view_count(view)?
                    };
                    counts.insert(view.clone(), count);
                    Ok(count)
                })
                .collect()
        })
    }

    fn navigation_totals(
        &self,
        views: &[View],
    ) -> Result<HashMap<View, usize>> {
        let mut counts = HashMap::new();
        let ordinary = [
            View::All,
            View::Inbox,
            View::Read,
            View::Starred,
            View::Queue,
            View::Trash,
            View::Annotated,
            View::Continue,
            View::Grouped,
        ];
        if views.iter().any(|view| ordinary.contains(view)) {
            let parameters = rusqlite::named_params! {
                ":annotated": views.contains(&View::Annotated),
                ":continuing": views.contains(&View::Continue),
                ":grouped": views.contains(&View::Grouped),
            };
            self.connection.query_row(
                include_str!("../sql/select_navigation_totals.sql"),
                parameters,
                |row| {
                    for (index, view) in ordinary.into_iter().enumerate() {
                        counts.insert(view, row.get(index)?);
                    }
                    Ok(())
                },
            )?;
        }
        if views.iter().any(|view| {
            matches!(
                view,
                View::Feed(_) | View::Server(_) | View::Collection(_)
            )
        }) {
            self.navigation_groups(&mut counts)?;
        }
        Ok(counts)
    }

    fn navigation_groups(
        &self,
        counts: &mut HashMap<View, usize>,
    ) -> Result<()> {
        let mut statement = self.connection.prepare_cached(include_str!(
            "../sql/select_navigation_groups.sql"
        ))?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let id = row.get(1)?;
            let view = match row.get::<_, String>(0)?.as_str() {
                "feed" => View::Feed(id),
                "server" => View::Server(id),
                _ => View::Collection(id),
            };
            counts.insert(view, row.get(2)?);
        }
        Ok(())
    }
}
