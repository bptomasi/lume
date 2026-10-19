use super::{Feature, Store};
use crate::{
    model::{RecommendationEvidence, RecommendationProfile},
    search::Expression,
};
use anyhow::Result;
use rusqlite::types::Value;

impl Store {
    pub(super) fn classify_dirty(
        &self,
        dirty: &[(i64, bool)],
        profile: &RecommendationProfile,
    ) -> Result<Vec<Feature>> {
        let analyze: Vec<_> = dirty
            .iter()
            .filter(|(_, text)| *text)
            .map(|(id, _)| *id)
            .collect();
        let reweight: Vec<_> = dirty
            .iter()
            .filter(|(_, text)| !*text)
            .map(|(id, _)| *id)
            .collect();
        self.classify_matches(&analyze, profile)?;
        let mut features = self.read_evidence(&analyze)?;
        let excluded = self.matching_ids(&analyze, &profile.exclude, false)?;
        for feature in &mut features {
            feature.excluded = !profile.exclude.is_empty()
                && excluded.contains(&feature.article);
            let (topics, headlines) =
                self.matched_interests(feature.article)?;
            feature.evidence.interests = topics;
            feature.evidence.headline = headlines;
        }
        features.extend(self.read_reweights(&reweight)?);
        Ok(features)
    }

    fn classify_matches(
        &self,
        ids: &[i64],
        profile: &RecommendationProfile,
    ) -> Result<()> {
        for id in ids {
            self.connection.execute(
                "DELETE FROM recommendation_matches WHERE article_id = ?1",
                [id],
            )?;
        }
        for interest in &profile.interests {
            let matches = self.matching_ids(ids, &interest.query, false)?;
            let headlines = self.matching_ids(ids, &interest.query, true)?;
            for id in matches {
                self.connection.execute(
                    include_str!("../sql/recommendations/insert_match.sql"),
                    (id, &interest.name, headlines.contains(&id)),
                )?;
            }
        }
        Ok(())
    }

    fn matching_ids(
        &self,
        ids: &[i64],
        query: &str,
        headline: bool,
    ) -> Result<Vec<i64>> {
        if ids.is_empty() || query.is_empty() {
            return Ok(Vec::new());
        }
        let mut expression = crate::search::parse(query)?;
        if headline && !title_expression(&mut expression) {
            return Ok(Vec::new());
        }
        let first = ids.iter().min().copied().unwrap_or(0);
        let last = ids.iter().max().copied().unwrap_or(0);
        let compiled = if last - first <= 255 {
            crate::search::compile::compile_publisher_range(
                &expression,
                first,
                last,
            )?
        } else {
            crate::search::compile::compile_publisher(&expression)?
        };
        let sql = format!(
            "{}\nAND ({})",
            include_str!("../sql/recommendations/select_match_batch.sql"),
            compiled.sql
        );
        let mut statement = self.connection.prepare_cached(&sql)?;
        if let Some(index) = statement.parameter_index(":ids")? {
            statement.raw_bind_parameter(index, serde_json::to_string(ids)?)?;
        }
        bind_search(&mut statement, compiled.values)?;
        Ok(statement
            .raw_query()
            .mapped(|row| row.get(0))
            .collect::<rusqlite::Result<_>>()?)
    }

    fn matched_interests(&self, id: i64) -> Result<(Vec<String>, Vec<String>)> {
        let pairs = self
            .connection
            .prepare_cached(include_str!(
                "../sql/recommendations/select_matches.sql"
            ))?
            .query_map([id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let headline = pairs
            .iter()
            .filter(|(_, yes)| *yes)
            .map(|(name, _)| name.clone())
            .collect();
        Ok((pairs.into_iter().map(|(name, _)| name).collect(), headline))
    }

    fn read_evidence(&self, ids: &[i64]) -> Result<Vec<Feature>> {
        let mut statement = self.connection.prepare_cached(include_str!(
            "../sql/recommendations/select_evidence.sql"
        ))?;
        let mut rows = statement.query([serde_json::to_string(ids)?])?;
        let mut features = Vec::new();
        while let Some(row) = rows.next()? {
            features.push(read_feature(row)?);
        }
        Ok(features)
    }

    fn read_reweights(&self, ids: &[i64]) -> Result<Vec<Feature>> {
        self.connection
            .prepare_cached(include_str!(
                "../sql/recommendations/select_reweight.sql"
            ))?
            .query_map([serde_json::to_string(ids)?], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, usize>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, bool>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .map(|row| {
                let (article, minutes, text, excluded, starred, feed) = row?;
                Ok(Feature {
                    article,
                    minutes,
                    evidence: serde_json::from_str(&text)?,
                    excluded,
                    starred,
                    feed,
                })
            })
            .collect()
    }
}

fn title_expression(expression: &mut Expression) -> bool {
    match expression {
        Expression::All => false,
        Expression::And(left, right) | Expression::Or(left, right) => {
            title_expression(left) & title_expression(right)
        }
        Expression::Not(_) => false,
        Expression::Term { field, .. } => {
            if !matches!(field.as_str(), "" | "topic" | "body" | "title") {
                return false;
            }
            *field = "title".into();
            true
        }
    }
}

fn bind_search(
    statement: &mut rusqlite::Statement<'_>,
    values: Vec<Value>,
) -> Result<()> {
    for (index, value) in values.iter().enumerate() {
        if let Some(parameter) =
            statement.parameter_index(&format!(":search_{index}"))?
        {
            statement.raw_bind_parameter(parameter, value)?;
        }
    }
    Ok(())
}

fn has_code(text: &str) -> bool {
    pulldown_cmark::Parser::new(text).any(|event| {
        matches!(
            event,
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::CodeBlock(_))
        )
    })
}

fn read_feature(row: &rusqlite::Row<'_>) -> Result<Feature> {
    let feed: String = row.get(5)?;
    let full: String = row.get(6)?;
    let origin: String = row.get(4)?;
    let source = evaluated_from(&origin, &feed, &full);
    let title = row.get::<_, String>(1)?.to_lowercase();
    let announcement = ["announcing ", "introducing ", "webinar", "sponsored"]
        .iter()
        .any(|word| title.contains(word));
    Ok(Feature {
        article: row.get(0)?,
        minutes: if full.is_empty() {
            row.get(2)?
        } else {
            crate::content::reading_minutes(&full)
        },
        evidence: RecommendationEvidence {
            evaluated_from: source.into(),
            words: feed.split_whitespace().count()
                + full.split_whitespace().count(),
            code: has_code(&feed) || has_code(&full),
            announcement,
            ..Default::default()
        },
        excluded: false,
        starred: row.get(3)?,
        feed: row.get(7)?,
    })
}

fn evaluated_from(origin: &str, feed: &str, full: &str) -> &'static str {
    if !full.is_empty() {
        return "Feed + cached full article";
    }
    match crate::content::body_kind(feed) {
        crate::content::BodyKind::Missing => "Title only",
        crate::content::BodyKind::ReferencesOnly => "Title + feed references",
        crate::content::BodyKind::Text => {
            if origin == "summary" {
                "Feed summary"
            } else {
                "Feed content"
            }
        }
    }
}
