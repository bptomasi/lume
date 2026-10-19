//! Review choices expose fixed intervals and keep postponement distinct.

use anyhow::Result;

use super::super::{App, Choice, ChoiceAction, StudyAction};

/// Captured study material, retained from question through grading.
#[derive(Clone, Debug)]
pub enum ReviewTarget {
    /// User-authored card with its expected schedule.
    Card(Box<crate::model::StudyCard>),
    /// User-authored note and its expected source/schedule.
    Note(Box<crate::model::AnnotationSnapshot>),
}

impl App {
    pub(in crate::app) fn choose_review_grade(
        &mut self,
        target: ReviewTarget,
    ) -> Result<()> {
        use crate::model::ReviewGrade as G;
        let choices = [G::Again, G::Hard, G::Good, G::Easy].into_iter()
            .map(|grade| Choice {
                title: format!("{grade:?} · {} days", grade.days()),
                detail: "Record one completed review and set this explicit interval. No adaptive scheduling model is used.".into(),
                action: ChoiceAction::Study(StudyAction::Grade {
                    target: target.clone(),
                    days: grade.days(),
                    postpone: false,
                }),
            }).chain(std::iter::once(Choice {
                title: "Postpone · 1 day".into(),
                detail: "Keep the completed-review count unchanged and revisit tomorrow.".into(),
                action: ChoiceAction::Study(StudyAction::Grade {
                    target: target.clone(),
                    days: 1,
                    postpone: true,
                }),
            })).collect();
        self.inspect(choices)
    }

    pub(in crate::app) fn grade_review(
        &mut self,
        target: ReviewTarget,
        days: u16,
        postpone: bool,
    ) -> Result<()> {
        if let Some(writer) = &mut self.note_tasks {
            writer.submit(super::super::note_tasks::Task::Edit(
                crate::app::NoteEdit::Review {
                    target,
                    days,
                    postpone,
                },
            ))?;
        } else {
            let article = super::super::note_tasks::review(
                &self.store,
                &target,
                days,
                postpone,
            )?;
            self.refresh_notes(article)?;
        }
        self.status = if postpone {
            format!(
                "Postponed for {days} days · review count unchanged · u undo"
            )
        } else {
            format!("Review recorded · due in {days} days · u undo")
        };
        Ok(())
    }
}

pub(super) fn note_choices(
    store: &crate::store::Store,
    query: &str,
) -> Result<Vec<Choice>> {
    let mut choices = Vec::new();
    let query = if query.trim().is_empty() {
        "is:due".into()
    } else {
        format!("is:due ({query})")
    };
    for note in store.search_notes(&query, None, 0, 100)?.notes {
        let annotation = store.annotation(note.article, note.id)?;
        choices.push(Choice {
            title: crate::content::label(
                if annotation.comment.quote.is_empty() {
                    &annotation.comment.body
                } else {
                    &annotation.comment.quote
                },
                120,
            ),
            detail: format!(
                "{}\n\n{}\n\n{}",
                annotation.comment.quote,
                annotation.comment.body,
                "Enter chooses Again / Hard / Good / Easy or Postpone",
            ),
            action: ChoiceAction::Study(StudyAction::ReviewNote(Box::new(
                annotation,
            ))),
        });
    }
    Ok(choices)
}

pub(super) fn card_choices(
    store: &crate::store::Store,
    query: &str,
) -> Result<Vec<Choice>> {
    let choices = store
        .filtered_study_cards(query, chrono::Utc::now().timestamp())?
        .into_iter()
        .map(|card| Choice {
            title: card.question.clone(),
            detail: "Recall the answer, then Enter reveals it.\n\
                    Nothing is marked reviewed yet."
                .into(),
            action: ChoiceAction::Study(StudyAction::Reveal(Box::new(card))),
        })
        .collect();
    Ok(choices)
}
