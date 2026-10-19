use super::super::Store;
use crate::model::RecommendationProfile;
use anyhow::Result;
use rusqlite::OptionalExtension;

impl Store {
    /// Read saved preferences or the explicit bundled computing interests.
    ///
    /// # Errors
    /// Returns invalid persisted settings or SQLite errors.
    pub fn recommendation_profile(&self) -> Result<RecommendationProfile> {
        let text: Option<String> = self.connection.query_row(
            "SELECT value FROM settings WHERE key = 'recommendation_profile'",
            [], |row| row.get(0)).optional()?;
        let profile = text
            .map(|text| serde_json::from_str(&text))
            .transpose()?
            .unwrap_or_default();
        crate::recommendation::validate_profile(&profile)?;
        Ok(profile)
    }

    /// Atomically replace preferences and invalidate previous text evidence.
    ///
    /// # Errors
    /// Rejects invalid profiles and reports SQLite errors without partial changes.
    pub fn save_recommendation_profile(
        &self,
        profile: &RecommendationProfile,
    ) -> Result<()> {
        crate::recommendation::validate_profile(profile)?;
        let transaction = self.connection.unchecked_transaction()?;
        self.connection.execute(
            include_str!("../sql/recommendations/insert_profile.sql"),
            [serde_json::to_string(profile)?],
        )?;
        self.connection.execute_batch(include_str!(
            "../sql/recommendations/update_profile_cache.sql"
        ))?;
        self.connection.execute(
            include_str!("../sql/recommendations/insert_all_dirty.sql"),
            [1],
        )?;
        transaction.commit()?;
        Ok(())
    }
}
