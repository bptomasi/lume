//! Incremental publisher matching, durable feedback and bounded ranked snapshots.
//!
//! Content/state triggers enqueue identities in the publisher transaction. Each
//! refresh owns one SQLite snapshot and at most 64 classifications. Unchanged
//! articles reuse evidence. Publication uses at most 512 metadata-only rows;
//! navigation reads the persisted shortlist and never performs classification.

mod classify;
mod feedback;
mod profile;
mod publish;

use super::Store;
use crate::model::{RecommendationEvidence, RecommendationUpdate};
use anyhow::Result;

struct Feature {
    article: i64,
    minutes: usize,
    evidence: RecommendationEvidence,
    excluded: bool,
    starred: bool,
    feed: i64,
}

impl Store {
    /// Refresh at most 64 dirty articles and publish a diverse cached shortlist.
    ///
    /// # Errors
    /// Returns validation/SQLite errors; the complete batch rolls back on failure.
    pub fn refresh_recommendations(&self) -> Result<RecommendationUpdate> {
        if let Some(idle) = self.idle_recommendations()? {
            return Ok(idle);
        }
        let transaction = self.write_transaction()?;
        let dirty = self
            .connection
            .prepare_cached(include_str!(
                "sql/recommendations/select_dirty.sql"
            ))?
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, bool>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if dirty.is_empty() {
            let idle = self.recommendation_idle_state()?;
            transaction.commit()?;
            return Ok(idle);
        }
        let profile = self.recommendation_profile()?;
        let changed = self.classify_dirty(&dirty, &profile)?;
        self.write_features(changed, &profile)?;
        for (id, _) in &dirty {
            self.connection.execute(
                "DELETE FROM recommendation_dirty WHERE article_id = ?1",
                [id],
            )?;
        }
        let total = self.publish_recommendations(&profile)?;
        let remaining = self.recommendations_dirty()?;
        transaction.commit()?;
        Ok(RecommendationUpdate {
            processed: dirty.len(),
            remaining,
            total,
        })
    }

    // Idle polls remain ordinary WAL readers. A real batch reserves write
    // authority before capturing its dirty queue and evidence dependencies.
    fn idle_recommendations(&self) -> Result<Option<RecommendationUpdate>> {
        self.read_snapshot(|store| {
            if store.recommendations_dirty()? {
                return Ok(None);
            }
            store.recommendation_idle_state().map(Some)
        })
    }

    fn recommendation_idle_state(&self) -> Result<RecommendationUpdate> {
        let total = self
            .connection
            .prepare_cached(include_str!(
                "sql/recommendations/select_total.sql"
            ))?
            .query_row([], |row| row.get(0))?;
        Ok(RecommendationUpdate {
            processed: 0,
            remaining: false,
            total,
        })
    }

    /// Check the indexed dirty queue without scanning article content.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn recommendations_dirty(&self) -> Result<bool> {
        Ok(self.connection.query_row(
            include_str!("sql/recommendations/select_dirty_exists.sql"),
            [],
            |row| row.get(0),
        )?)
    }

    /// Explain cached evidence for any already classified article.
    ///
    /// # Errors
    /// Returns an absent/stale classification or SQLite/JSON error.
    pub fn recommendation_evidence(
        &self,
        article: i64,
    ) -> Result<RecommendationEvidence> {
        let text: String = self.connection.query_row(
            include_str!("sql/recommendations/select_current_evidence.sql"),
            [article],
            |row| row.get(0),
        )?;
        Ok(serde_json::from_str(&text)?)
    }
}
