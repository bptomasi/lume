//! User-authored study workflows capture ownership before opening editors.

use anyhow::{Context, Result};

use crate::model::StudyCard;

use super::{Action, App, Choice, ChoiceAction, InputKind};

/// Captured study action, independent of subsequent article selection.
#[derive(Clone, Debug)]
pub enum StudyAction {
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
            "review" => self.show_review()?,
            "cards" => self.show_study_cards()?,
            "card" => self.open_card(argument)?,
            "review-in" => {
                let (article, comment) = self.selected_annotation()?;
                self.store.review_annotation(
                    article,
                    comment,
                    argument.parse()?,
                    chrono::Utc::now().timestamp(),
                )?;
                self.status = format!(
                    "Passage scheduled for review in {argument} days · u undo"
                );
            }
            "link-note" => {
                let (article, comment) = self.selected_annotation()?;
                self.store.link_annotation(
                    article,
                    comment,
                    argument.parse()?,
                )?;
                self.status =
                    "Related article linked to the selected note · u undo"
                        .into();
            }
            "note-links" => self.show_note_links()?,
            "export-notes" => self.export_incremental(argument)?,
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    fn open_card(&mut self, argument: &str) -> Result<()> {
        let (article, comment) = self.selected_annotation()?;
        if argument.is_empty() {
            self.begin_input(
                InputKind::StudyCard { article, comment },
                String::new(),
            );
        } else {
            self.save_study_card(article, comment, argument)?;
        }
        Ok(())
    }

    fn selected_annotation(&self) -> Result<(i64, i64)> {
        Ok((
            self.current().context("Select an article")?.id,
            self.comments
                .get(self.comment_selected)
                .context("Select a note/highlight with [ or ]")?
                .id,
        ))
    }

    pub(super) fn save_study_card(
        &mut self,
        article: i64,
        comment: i64,
        text: &str,
    ) -> Result<()> {
        let (question, answer) = text.split_once('|').context(
            "Use Question | Answer; both must be your own study text",
        )?;
        self.store
            .create_study_card(article, comment, question, answer)?;
        self.status =
            "User-authored card saved · :cards reviews due questions · u undo"
                .into();
        Ok(())
    }

    fn show_review(&mut self) -> Result<()> {
        let mut choices = Vec::new();
        for (article, comment) in
            self.store.due_annotations(chrono::Utc::now().timestamp())?
        {
            let annotation = self.store.annotation(article, comment)?;
            choices.push(Choice {
                title: crate::content::label(&annotation.comment.quote, 120),
                detail: format!(
                    "{}\n\n{}\n\n{}",
                    annotation.comment.quote,
                    annotation.comment.body,
                    "Enter: reviewed, remind me in 7 days · :review-in DAYS",
                ),
                action: ChoiceAction::Study(StudyAction::ReviewNote(Box::new(
                    annotation,
                ))),
            });
        }
        self.inspect(choices)
    }

    fn show_study_cards(&mut self) -> Result<()> {
        let choices = self
            .store
            .due_study_cards(chrono::Utc::now().timestamp())?
            .into_iter()
            .map(|card| Choice {
                title: card.question.clone(),
                detail: "Recall the answer, then Enter reveals it.\n\
                    Nothing is marked reviewed yet."
                    .into(),
                action: ChoiceAction::Study(StudyAction::Reveal(Box::new(
                    card,
                ))),
            })
            .collect();
        self.inspect(choices)
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
            StudyAction::Reveal(card) => {
                let text = format!("{}\n\n{}", card.question, card.answer);
                self.show_document(
                    "Study answer · Enter reviewed / 7 days · Esc leaves due",
                    &text,
                    Some(ChoiceAction::Study(StudyAction::Review(card))),
                );
            }
            StudyAction::Review(card) => {
                self.store.review_study_card(
                    &card,
                    7,
                    chrono::Utc::now().timestamp(),
                )?;
                self.status = "Card reviewed · due in 7 days · u undo".into();
            }
            StudyAction::ReviewNote(annotation) => {
                self.store.review_captured_annotation(
                    &annotation,
                    7,
                    chrono::Utc::now().timestamp(),
                )?;
                self.status =
                    "Passage reviewed · due in 7 days · u undo".into();
            }
        }
        Ok(())
    }

    fn export_incremental(&mut self, directory: &str) -> Result<()> {
        anyhow::ensure!(!directory.is_empty(), "Use :export-notes DIRECTORY");
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
