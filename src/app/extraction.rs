//! Source diagnostics and explicit, captured extraction-rule confirmation.

use crate::app::commands::routing;

use anyhow::{Context, Result, ensure};

use crate::model::{RulePreview, RuleRequest, SourceHealth};

use super::{Action, App, Choice, ChoiceAction, Mode, PickerKind};

impl App {
    /// Indicate preview work without changing preferred source or retry state.
    pub fn rule_preview_started(
        &mut self,
        target: crate::model::ArticleTarget,
    ) {
        self.article_requests.start_preview(target);
        self.interaction
            .report("Testing selector · rule remains unsaved".into());
    }

    pub(super) fn extraction_command(
        &mut self,
        command: routing::ExtractionCommand,
        argument: &str,
    ) -> Result<Action> {
        match command {
            routing::ExtractionCommand::ExtractPreview => {
                ensure!(
                    self.article_requests.fetching().is_none(),
                    "Article worker is busy"
                );
                let target = self.selected_article_target()?;
                let feed = self.current().context("Select an article")?.feed_id;
                let selector = if argument == "auto" { "" } else { argument };
                crate::validation::validate_selector(selector)?;
                return Ok(Action::PreviewRule(RuleRequest {
                    target,
                    feed,
                    previous: self.store.article_selector(feed)?,
                    selector: selector.to_owned(),
                }));
            }
            routing::ExtractionCommand::SourceHealth => {
                self.show_source_health()?
            }
        }
        Ok(Action::None)
    }

    /// Stage a successful unsaved preview without replacing an active editor.
    pub fn rule_preview_finished(&mut self, preview: RulePreview) {
        if !self.article_requests.finish_preview(preview) {
            return;
        }
        self.interaction
            .report("Extraction preview ready · not saved".into());
    }

    /// Present a pending preview only when the user is no longer editing.
    pub fn present_rule_preview(&mut self) -> bool {
        if !matches!(self.interaction.mode(), Mode::Normal)
            || self.reader.selecting()
        {
            return false;
        }
        let Some(preview) = self.article_requests.take_preview() else {
            return false;
        };
        let warnings =
            crate::source::article::rules::warnings(&preview.document.body);
        let detail = format!(
            "UNSAVED · source {} · article {}\nRule: {}\n{}\n\n{}",
            preview.feed,
            preview.target.article,
            if preview.selector.is_empty() {
                "Readability"
            } else {
                &preview.selector
            },
            warnings.join("\n"),
            preview.document.body,
        );
        self.show_document(
            "Unsaved extraction · Enter saves · Esc discards · j/k scroll",
            &detail,
            Some(ChoiceAction::SaveRule(Box::new(preview))),
        );
        true
    }

    fn show_source_health(&mut self) -> Result<()> {
        let choices = self
            .store
            .source_health()?
            .into_iter()
            .map(source_health_choice)
            .collect();
        self.inspect(choices)
    }

    pub(super) fn inspect(&mut self, choices: Vec<Choice>) -> Result<()> {
        self.open_picker(PickerKind::Inspection)?;
        if let Some(picker) = self.interaction.picker_mut() {
            picker.replace_candidates(choices);
        }
        self.update_picker()
    }
}

fn source_health_choice(health: SourceHealth) -> Choice {
    let state = source_health_state(&health);
    let selector = if health.selector.is_empty() {
        "Readability"
    } else {
        &health.selector
    };
    Choice {
        title: format!("{} · {state}", health.title),
        detail: format!(
            "{}\n\nLast attempt: {}\nLast success: {}\nRetry: {}\n\
            Consecutive failures: {}\nSelector: {}\n\n{}",
            health.url,
            timestamp(health.attempted_at),
            timestamp(health.succeeded_at),
            timestamp(health.retry_at),
            health.failures,
            selector,
            health.error,
        ),
        action: ChoiceAction::SetView(crate::model::View::Feed(health.feed)),
    }
}

fn source_health_state(health: &SourceHealth) -> &'static str {
    if !health.error.is_empty() {
        return "FAILED";
    }
    if health.retry_at > chrono::Utc::now().timestamp() {
        return "WAITING";
    }
    if health.attempted_at == 0 {
        return "NEW";
    }
    "OK"
}

fn timestamp(value: i64) -> String {
    if value == 0 {
        return "Not recorded / ready on next refresh".into();
    }
    chrono::DateTime::from_timestamp(value, 0).map_or_else(
        || "Invalid timestamp".into(),
        |date| date.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
    )
}
