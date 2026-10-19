//! Small preference commands reuse Lume's existing boolean search vocabulary.

use super::{
    super::{Action, App},
    Edit,
};
use crate::model::{Interest, RecommendationFeedback, View};
use anyhow::{Context, Result, ensure};

impl App {
    pub(in crate::app) fn recommendation_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        ensure!(
            self.notebook.is_none()
                || !matches!(
                    verb,
                    "recommend-feedback" | "recommend-why" | "similar"
                ),
            "Open the source with Enter before using article actions"
        );
        match verb {
            "recommendations" => {
                self.set_view(View::Recommended)?;
                self.focus = super::super::Focus::Articles;
                self.status = "For you · :recommend-why explains · :recommend-feedback rates".into();
            }
            "recommend-profile" => self.show_recommendation_profile()?,
            "interest" => self.edit_recommendations(Edit::Interest(
                parse_interest(argument)?,
            ))?,
            "interest-remove" => {
                ensure!(!argument.is_empty(), "Use :interest-remove NAME");
                self.edit_recommendations(Edit::Remove(argument.into()))?;
            }
            "recommend-exclude" => {
                self.edit_recommendations(Edit::Exclude(argument.into()))?
            }
            "recommend-settings" => self.recommendation_setting(argument)?,
            "recommend-feedback" => self.rate_recommendation(argument)?,
            "recommend-why" => self.explain_recommendation()?,
            "similar" => self.similar_interests()?,
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    fn show_recommendation_profile(&mut self) -> Result<()> {
        let profile = self.store.recommendation_profile()?;
        let text = format!(
            "# Recommendation profile\n\nLocal, explicit interests; read state is not approval.\n\n```json\n{}\n```\n\n\
             Edit: :interest NAME | PRIORITY | QUERY\n\n\
             Remove: :interest-remove NAME\n\n\
             Exclude: :recommend-exclude QUERY (empty clears)\n\n\
             Settings: :recommend-settings minutes 20; read on/off; source ID WEIGHT; reset\n\n\
             Use :help recommendations for examples and limits.",
            serde_json::to_string_pretty(&profile)?
        );
        self.show_document("Recommendation profile · Esc closes", &text, None);
        Ok(())
    }

    fn recommendation_setting(&mut self, argument: &str) -> Result<()> {
        let parts =
            shlex::split(argument).context("Invalid settings quoting")?;
        let edit = match parts
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .as_slice()
        {
            ["minutes", "off"] => Edit::Minutes(None),
            ["minutes", value] => Edit::Minutes(Some(value.parse()?)),
            ["read", "on"] => Edit::IncludeRead(true),
            ["read", "off"] => Edit::IncludeRead(false),
            ["source", id, weight] => {
                Edit::Source(id.parse()?, weight.parse()?)
            }
            ["reset"] => Edit::Reset,
            _ => anyhow::bail!(
                "Use minutes 1–240/off, read on/off, source ID -10..10, or reset"
            ),
        };
        self.edit_recommendations(edit)
    }

    fn rate_recommendation(&mut self, argument: &str) -> Result<()> {
        self.guard_notebook_article_command("recommend-feedback")?;
        let article = self.current().context("Select an article first")?.id;
        let feedback = match argument {
            "useful" => Some(RecommendationFeedback::Useful),
            "not-interested" => Some(RecommendationFeedback::NotInterested),
            "too-basic" => Some(RecommendationFeedback::TooBasic),
            "too-advanced" => Some(RecommendationFeedback::TooAdvanced),
            "clear" => None,
            _ => anyhow::bail!(
                "Use useful, not-interested, too-basic, too-advanced or clear"
            ),
        };
        self.edit_recommendations(Edit::Feedback(article, feedback))
    }

    fn explain_recommendation(&mut self) -> Result<()> {
        let article = self.current().context("Select an article first")?;
        let evidence = self.store.recommendation_evidence(article.id).context(
            "Evidence is not ready; indexing continues in the background",
        )?;
        let profile = self.store.recommendation_profile()?;
        let rating = self.store.recommendation_feedback(article.id)?;
        let text = format!(
            "# Why recommended\n\n{}\n\n\
            Matched interests: {}\n\nTitle matches: {}\n\n\
            Evaluated from: {}\n\nAvailable words: {}\n\n\
            Fenced code observed: {}\n\nAnnouncement wording: {}\n\n\
            Favorite: {}\n\nExplicit source preference: {}\n\nFeedback: {}\n\n\
            Priorities: {}\n\n\
            Feedback affinity has a bounded influence. Source/topic repetition\n\
            reduces prominence in the shortlist. Scores estimate relevance;\n\
            they do not verify correctness or technical depth.",
            article.title,
            joined(&evidence.interests),
            joined(&evidence.headline),
            evidence.evaluated_from,
            evidence.words,
            evidence.code,
            evidence.announcement,
            article.starred,
            profile.sources.get(&article.feed_id).copied().unwrap_or(0),
            rating.map_or("None", RecommendationFeedback::as_str),
            profile
                .interests
                .iter()
                .filter(|interest| evidence.interests.contains(&interest.name))
                .map(|interest| format!(
                    "{} ({})",
                    interest.name, interest.weight
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
        self.show_document("Why recommended · Esc closes", &text, None);
        Ok(())
    }

    fn similar_interests(&mut self) -> Result<()> {
        let article = self.current().context("Select an article first")?.id;
        let evidence = self.store.recommendation_evidence(article)?;
        let profile = self.store.recommendation_profile()?;
        let mut interests: Vec<_> = profile
            .interests
            .iter()
            .filter(|interest| evidence.interests.contains(&interest.name))
            .collect();
        interests.sort_by_key(|interest| std::cmp::Reverse(interest.weight));
        let query = interests
            .iter()
            .take(4)
            .map(|interest| format!("({})", interest.query))
            .collect::<Vec<_>>()
            .join(" OR ");
        ensure!(
            !query.is_empty(),
            "No shared interests; configure :interest first"
        );
        crate::search::parse(&query)?;
        self.set_view(View::Recommended)?;
        self.search = query;
        self.filter()?;
        self.focus = super::super::Focus::Articles;
        self.status =
            "More like this · strongest shared interests within For you".into();
        Ok(())
    }
}

fn parse_interest(argument: &str) -> Result<Interest> {
    let fields = argument.splitn(3, '|').map(str::trim).collect::<Vec<_>>();
    ensure!(
        fields.len() == 3,
        "Use :interest NAME | PRIORITY 1–10 | QUERY"
    );
    let interest = Interest {
        name: fields[0].into(),
        weight: fields[1].parse()?,
        query: fields[2].into(),
    };
    Ok(interest)
}

fn joined(values: &[String]) -> String {
    if values.is_empty() {
        return "None".into();
    }
    values.join(", ")
}
