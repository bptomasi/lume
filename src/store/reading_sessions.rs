//! Reading sessions use explicit checkpoints, never inferred positive feedback.

use anyhow::{Result, ensure};
use rusqlite::OptionalExtension;

use super::Store;
use crate::model::{ReadingSession, RecommendationEvidence, SessionItem};

impl Store {
    /// Build a proposal from the current cached unread recommendation shortlist.
    ///
    /// # Errors
    /// Returns invalid budgets, recommendation metadata or storage errors.
    pub fn plan_reading_session(&self, budget: u16) -> Result<ReadingSession> {
        let mut statement = self.connection.prepare_cached(include_str!(
            "sql/select_session_candidates.sql"
        ))?;
        let candidates = statement
            .query_map([], |row| {
                let evidence: String = row.get(7)?;
                Ok((
                    SessionItem {
                        article: row.get(0)?,
                        feed: row.get(1)?,
                        url: row.get(2)?,
                        canonical: row.get(3)?,
                        title: row.get(4)?,
                        minutes: row.get::<_, u16>(5)?.max(1),
                        score: row.get(6)?,
                        topic: String::new(),
                    },
                    evidence,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let candidates = candidates
            .into_iter()
            .map(|(mut item, evidence)| {
                let evidence: RecommendationEvidence =
                    serde_json::from_str(&evidence)?;
                item.topic =
                    evidence.interests.first().cloned().unwrap_or_default();
                item.title = crate::content::label(&item.title, 200);
                Ok(item)
            })
            .collect::<Result<Vec<_>>>()?;
        crate::recommendation::session::plan(candidates, budget)
    }

    /// Read an existing sequence, including completed sessions.
    ///
    /// # Errors
    /// Returns invalid checkpoint or SQLite errors.
    pub fn reading_session(&self) -> Result<Option<ReadingSession>> {
        let value: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'reading_session'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        value
            .map(|value| {
                ensure!(
                    value.len() <= 131_072,
                    "Session checkpoint exceeds 128 KiB"
                );
                let session: ReadingSession = serde_json::from_str(&value)?;
                session.validate()?;
                Ok(session)
            })
            .transpose()
    }

    /// Save an explicit plan without modifying articles or reading state.
    ///
    /// # Errors
    /// Returns invalid plans or storage errors.
    pub fn save_reading_session(&self, session: &ReadingSession) -> Result<()> {
        session.validate()?;
        let value = serde_json::to_string(session)?;
        ensure!(value.len() <= 131_072, "Session checkpoint exceeds 128 KiB");
        self.connection.execute(
            r#"
            INSERT INTO settings (key, value)
            VALUES ('reading_session', ?1)
            ON CONFLICT (key) DO UPDATE SET value = excluded.value
            "#,
            [value],
        )?;
        Ok(())
    }

    /// Remove only the reading sequence; articles and personal data remain.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn end_reading_session(&self) -> Result<()> {
        self.connection.execute(
            "DELETE FROM settings WHERE key = 'reading_session'",
            [],
        )?;
        Ok(())
    }
}
