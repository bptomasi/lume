use super::Store;
use crate::model::RecommendationFeedback;
use anyhow::{Result, ensure};
use rusqlite::OptionalExtension;

impl Store {
    /// Record or clear feedback with captured interests; never infer it from read state.
    ///
    /// # Errors
    /// Rejects missing/deleted articles and reports SQLite errors atomically.
    pub fn set_recommendation_feedback(
        &self,
        article: i64,
        feedback: Option<RecommendationFeedback>,
    ) -> Result<()> {
        let transaction = self.write_transaction()?;
        ensure!(
            !self.is_deleted(article)?,
            "Restore the article before rating it"
        );
        let profile = self.recommendation_profile()?;
        let features = self.classify_dirty(&[(article, true)], &profile)?;
        self.write_features(features, &profile)?;
        // Invalidate both old and new affinities when a rating changes.
        self.connection.execute(
            include_str!("../sql/recommendations/insert_feedback_dirty.sql"),
            [article],
        )?;
        self.connection.execute(
            "DELETE FROM recommendation_feedback WHERE article_id = ?1",
            [article],
        )?;
        self.write_feedback(article, feedback)?;
        self.connection.execute(
            include_str!("../sql/recommendations/insert_feedback_dirty.sql"),
            [article],
        )?;
        self.connection.execute(
            include_str!("../sql/recommendations/insert_state_dirty.sql"),
            [article],
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn write_feedback(
        &self,
        article: i64,
        feedback: Option<RecommendationFeedback>,
    ) -> Result<()> {
        if let Some(feedback) = feedback {
            self.connection.execute(
                include_str!("../sql/recommendations/insert_feedback.sql"),
                (article, feedback.as_str()),
            )?;
            self.connection.execute(
                include_str!(
                    "../sql/recommendations/insert_feedback_topics.sql"
                ),
                [article],
            )?;
        }
        Ok(())
    }

    /// Read this article's explicit rating, independently of read/favorite state.
    ///
    /// # Errors
    /// Returns SQLite or invalid persisted feedback errors.
    pub fn recommendation_feedback(
        &self,
        article: i64,
    ) -> Result<Option<RecommendationFeedback>> {
        let name: Option<String> = self.connection.query_row(
            "SELECT feedback FROM recommendation_feedback WHERE article_id = ?1",
            [article], |row| row.get(0)).optional()?;
        name.map(|name| serde_json::from_value(serde_json::Value::String(name)))
            .transpose()
            .map_err(Into::into)
    }
}
