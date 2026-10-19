//! User-authored study workflows capture ownership before opening editors.

use anyhow::{Context, Result};

use crate::model::StudyCard;

use super::{Action, App, Choice, ChoiceAction, InputKind};

/// Captured study action, independent of subsequent article selection.
#[derive(Clone, Debug)]
pub enum StudyAction {
    /// Grade a revealed item or postpone it without claiming a review.
    Grade {
        /// Expected card or annotation snapshot.
        target: super::ReviewTarget,
        /// Explicit interval in days.
        days: u16,
        /// Defer without incrementing completed reviews.
        postpone: bool,
    },
    /// Reveal the captured card answer without marking it reviewed.
    Reveal(Box<StudyCard>),
    /// Record a review of the captured card.
    Review(Box<StudyCard>),
    /// Review one captured annotation after reading its preview.
    ReviewNote(Box<crate::model::AnnotationSnapshot>),
}

impl App {
    pub(super) fn study_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        match verb {
            "review" => self.show_review(argument)?,
            "cards" => self.show_study_cards(argument)?,
            "card" => self.open_card(argument)?,
            "review-in" => {
                let note = self.note_target()?;
                self.grade_review(
                    super::ReviewTarget::Note(Box::new(note)),
                    argument.parse()?,
                    false,
                )?;
            }
            "link-note" => {
                self.link_note_article(argument.parse()?)?;
            }
            "note-links" => self.show_note_links()?,
            "export-notes" => self.export_incremental(argument)?,
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    fn open_card(&mut self, argument: &str) -> Result<()> {
        let (article, comment) = self.selected_annotation()?;
        if argument.is_empty() || self.note_tasks.is_some() {
            self.ensure_no_recovery(&crate::model::DraftTarget::Card {
                article,
                note: comment,
                card: None,
            })?;
            self.begin_input(
                InputKind::StudyCard {
                    article,
                    comment,
                    card: None,
                },
                argument.into(),
            );
            if !argument.is_empty() {
                if let super::Mode::Input(input) = &mut self.mode {
                    input.saved_text.clear();
                }
                self.request_draft_write(
                    super::draft_commands::Destination::Reader,
                )?;
            }
        } else {
            self.save_study_card(article, comment, None, argument)?;
        }
        Ok(())
    }

    fn link_note_article(&mut self, related: i64) -> Result<()> {
        let (article, comment) = self.selected_annotation()?;
        if let Some(writer) = &mut self.note_tasks {
            writer.submit(super::note_tasks::Task::LinkArticle {
                article,
                note: comment,
                related,
            })?;
        } else {
            self.store.link_annotation(article, comment, related)?;
            self.refresh_notes(article)?;
        }
        self.status =
            "Related article linked to the selected note · u undo".into();
        Ok(())
    }

    fn selected_annotation(&self) -> Result<(i64, i64)> {
        let target = self.note_target()?;
        Ok((target.article, target.comment.id))
    }

    pub(super) fn save_study_card(
        &mut self,
        article: i64,
        comment: i64,
        card: Option<i64>,
        text: &str,
    ) -> Result<i64> {
        let (question, answer) = text.split_once('|').context(
            "Use Question | Answer; both must be your own study text",
        )?;
        let id = if let Some(card) = card {
            self.store
                .update_study_card(article, comment, card, question, answer)?
        } else {
            self.store
                .create_study_card(article, comment, question, answer)?
        };
        self.status =
            "User-authored card saved · :cards reviews due questions · u undo"
                .into();
        Ok(id)
    }

    fn show_review(&mut self, query: &str) -> Result<()> {
        self.inspect_notes(
            super::notebook::inspection::Inspection::ReviewNotes(query.into()),
        )
    }

    fn show_study_cards(&mut self, query: &str) -> Result<()> {
        self.inspect_notes(
            super::notebook::inspection::Inspection::ReviewCards(query.into()),
        )
    }

    fn show_note_links(&mut self) -> Result<()> {
        let (article, comment) = self.selected_annotation()?;
        let annotation = self.store.annotation(article, comment)?;
        let choices = annotation
            .links
            .into_iter()
            .map(|related| {
                let article = self.store.article(related)?;
                Ok(Choice {
                    title: article.title,
                    detail: article.url,
                    action: ChoiceAction::JumpArticle(related),
                })
            })
            .collect::<Result<_>>()?;
        self.inspect(choices)
    }

    pub(super) fn study_choice(&mut self, action: StudyAction) -> Result<()> {
        match action {
            StudyAction::Grade {
                target,
                days,
                postpone,
            } => {
                self.grade_review(target, days, postpone)?;
            }
            StudyAction::Reveal(card) => {
                let text = format!("{}\n\n{}", card.question, card.answer);
                self.show_document(
                    "Study answer · Enter grades · Esc leaves due",
                    &text,
                    Some(ChoiceAction::Study(StudyAction::Review(card))),
                );
            }
            StudyAction::Review(card) => {
                self.choose_review_grade(super::ReviewTarget::Card(card))?;
            }
            StudyAction::ReviewNote(annotation) => {
                self.choose_review_grade(super::ReviewTarget::Note(
                    annotation,
                ))?;
            }
        }
        Ok(())
    }

    fn export_incremental(&mut self, directory: &str) -> Result<()> {
        anyhow::ensure!(!directory.is_empty(), "Use :export-notes DIRECTORY");
        if let Some(writer) = &mut self.note_tasks {
            writer.submit(super::note_tasks::Task::ExportArticles(
                directory.into(),
            ))?;
            self.status =
                "Exporting article notes… · navigation remains available"
                    .into();
            return Ok(());
        }
        let report = crate::exchange::incremental::notebook(
            &self.store,
            std::path::Path::new(directory),
        )?;
        self.status = format!(
            "Notes export · {} written · {} unchanged · {} local edits preserved",
            report.written,
            report.unchanged,
            report.conflicts.len()
        );
        if !report.conflicts.is_empty() {
            let conflicts = report
                .conflicts
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            self.show_document(
                "Export conflicts · files preserved · Esc close",
                &conflicts,
                None,
            );
        }
        Ok(())
    }
}
