use crate::{model::RecommendationProfile, search::Expression};
use anyhow::{Result, ensure};

/// Validate resource limits and the publisher-only search vocabulary.
///
/// # Errors
/// Rejects oversized, duplicate, state-dependent or private-note criteria.
pub fn validate_profile(profile: &RecommendationProfile) -> Result<()> {
    ensure!(
        profile.interests.len() <= 24,
        "At most 24 interests are allowed"
    );
    let mut names = std::collections::BTreeSet::new();
    for interest in &profile.interests {
        ensure!((1..=10).contains(&interest.weight), "Priority must be 1–10");
        ensure!(
            !interest.name.trim().is_empty()
                && interest.name.len() <= 64
                && !interest.name.chars().any(char::is_control),
            "Invalid interest name"
        );
        ensure!(
            names.insert(interest.name.to_lowercase()),
            "Duplicate interest name"
        );
        ensure!(!interest.query.trim().is_empty(), "Interest query is empty");
        validate_query(&interest.query)?;
    }
    validate_query(&profile.exclude)?;
    ensure!(
        profile.minutes.is_none_or(|n| (1..=240).contains(&n)),
        "Reading time must be 1–240 minutes or off"
    );
    ensure!(
        profile.sources.len() <= 128
            && profile
                .sources
                .iter()
                .all(|(id, weight)| *id > 0 && (-10..=10).contains(weight)),
        "Invalid source preferences"
    );
    Ok(())
}

fn validate_query(query: &str) -> Result<()> {
    ensure!(query.len() <= 1024, "Interest query exceeds 1 KiB");
    publisher_expression(&crate::search::parse(query)?)
}

fn publisher_expression(expression: &Expression) -> Result<()> {
    match expression {
        Expression::All => Ok(()),
        Expression::And(left, right) | Expression::Or(left, right) => {
            publisher_expression(left)?;
            publisher_expression(right)
        }
        Expression::Not(inner) => publisher_expression(inner),
        Expression::Term { field, .. } => {
            ensure!(
                matches!(
                    field.as_str(),
                    "" | "topic"
                        | "title"
                        | "body"
                        | "source"
                        | "author"
                        | "domain"
                        | "tag"
                ),
                "Interests accept publisher text, domain and article tags"
            );
            Ok(())
        }
    }
}
