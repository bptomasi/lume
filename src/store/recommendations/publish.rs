use super::{Feature, Store};
use crate::model::{RecommendationEvidence, RecommendationProfile};
use crate::recommendation::{Candidate, select};
use anyhow::Result;
use std::collections::BTreeMap;

impl Store {
    pub(super) fn write_features(
        &self,
        features: Vec<Feature>,
        profile: &RecommendationProfile,
    ) -> Result<()> {
        let affinity = self
            .connection
            .prepare_cached(include_str!(
                "../sql/recommendations/select_affinities.sql"
            ))?
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
        for feature in features {
            let score = score(&feature, profile, &affinity);
            self.connection.execute(
                include_str!("../sql/recommendations/insert_feature.sql"),
                (
                    feature.article,
                    score,
                    feature.minutes as i64,
                    serde_json::to_string(&feature.evidence)?,
                    feature.excluded,
                ),
            )?;
        }
        Ok(())
    }

    pub(super) fn publish_recommendations(
        &self,
        profile: &RecommendationProfile,
    ) -> Result<usize> {
        let candidates = self.recommendation_candidates(profile)?;
        let selected = select(candidates);
        self.connection
            .execute("DELETE FROM recommendation_items", [])?;
        for (position, id) in selected.iter().enumerate() {
            self.connection.execute(
                include_str!("../sql/recommendations/insert_item.sql"),
                (id, position as i64),
            )?;
        }
        Ok(selected.len())
    }
    fn recommendation_candidates(
        &self,
        profile: &RecommendationProfile,
    ) -> Result<Vec<Candidate>> {
        let rows = self
            .connection
            .prepare_cached(include_str!(
                "../sql/recommendations/select_candidates.sql"
            ))?
            .query_map((profile.include_read, profile.minutes), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|row| candidate(row, profile))
            .collect::<Result<Vec<_>>>()
    }
}

fn score(
    feature: &Feature,
    profile: &RecommendationProfile,
    affinity: &BTreeMap<String, i64>,
) -> i64 {
    let evidence = &feature.evidence;
    let mut score = 0;
    for interest in &profile.interests {
        if !evidence.interests.contains(&interest.name) {
            continue;
        }
        // Feedback has a bounded influence; explicit priorities stay dominant.
        let adjustment = affinity
            .get(&interest.name)
            .copied()
            .unwrap_or(0)
            .clamp(-3, 3);
        score += i64::from(interest.weight) * 10 + adjustment * 4;
        if evidence.headline.contains(&interest.name) {
            score += 12;
        }
    }
    if score == 0 {
        return 0;
    }
    score += i64::from(*profile.sources.get(&feature.feed).unwrap_or(&0)) * 5;
    score += i64::from(feature.starred) * 8 + i64::from(evidence.code) * 3;
    score -= i64::from(evidence.announcement) * 8;
    score -= i64::from(evidence.words < 80) * 10;
    score.max(1)
}

fn candidate(
    (article, feed, canonical, score, text, published): (
        i64,
        i64,
        String,
        i64,
        String,
        i64,
    ),
    profile: &RecommendationProfile,
) -> Result<Candidate> {
    let evidence: RecommendationEvidence = serde_json::from_str(&text)?;
    let topic = profile
        .interests
        .iter()
        .filter(|interest| evidence.interests.contains(&interest.name))
        .max_by_key(|interest| interest.weight)
        .map_or(String::new(), |interest| interest.name.clone());
    Ok(Candidate {
        article,
        feed,
        canonical,
        score,
        topic,
        published,
    })
}
