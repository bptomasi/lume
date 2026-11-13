//! Captured bulk actions, independent tags and personal navigation preferences.

use crate::app::commands::routing;

use anyhow::{Context, Result, bail, ensure};

use crate::model::{BulkEdit, TagTarget};

use super::{Action, App, Choice, ChoiceAction, View};

impl App {
    pub(super) fn organization_command(
        &mut self,
        command: routing::OrganizationCommand,
        argument: &str,
    ) -> Result<Action> {
        use routing::OrganizationCommand as C;

        match command {
            C::SelectArticle => self.toggle_article_selection()?,
            C::SelectPage => self.select_page()?,
            C::ClearSelection => self.nav.clear_marks(),
            C::Bulk => self.apply_bulk(argument)?,
            C::Tags => {
                let article = self.current().context("Select an article")?.id;
                self.edit_tags(TagTarget::Article(article), argument)?;
            }
            C::Related => self.show_occurrences()?,
            C::Pin => self.toggle_pin()?,
            C::Collapse => self.toggle_navigation_group(argument)?,
            C::MoveUp | C::MoveDown => {
                self.reorder_navigation(command == C::MoveDown)?
            }
            C::Continue => self.set_view(View::Continue)?,
            C::SinceLastVisit => self.set_view(View::SinceLastVisit(
                self.layout.workspace().last_visit,
            ))?,
            C::Grouped => self.set_view(View::Grouped)?,
        }
        Ok(Action::None)
    }

    fn toggle_article_selection(&mut self) -> Result<()> {
        self.nav.mark_current()?;
        self.interaction.report(format!(
            "{} selected · :bulk ACTION · :clear-selection",
            self.nav.marked().len()
        ));
        Ok(())
    }

    fn select_page(&mut self) -> Result<()> {
        self.nav.mark_page()?;
        self.interaction.report(format!(
            "{} captured articles selected",
            self.nav.marked().len()
        ));
        Ok(())
    }

    fn apply_bulk(&mut self, argument: &str) -> Result<()> {
        let edit = bulk_edit(argument, self.nav.library())?;
        if matches!(edit, BulkEdit::Unread(_))
            && let Some(article) = self
                .current()
                .filter(|article| self.nav.marked().contains(&article.id))
        {
            self.auto_read_manual_state(article.id);
        }
        self.submit_article_edit(self.nav.marked().clone(), edit)?;
        let count = self.nav.marked().len();
        self.nav.clear_marks();
        self.nav.retain_article(None);
        if !self.personal.enabled() {
            self.reload()?;
        }
        self.interaction.report(format!(
            "Updated {count} captured articles · u undoes the entire batch"
        ));
        Ok(())
    }

    fn edit_tags(&mut self, target: TagTarget, argument: &str) -> Result<()> {
        if argument.is_empty() {
            self.show_document(
                "Independent tags · Esc close",
                &self.store.tags(target)?.join("\n"),
                None,
            );
            return Ok(());
        }
        let tags = if argument == "clear" {
            Vec::new()
        } else {
            argument.split_whitespace().map(str::to_owned).collect()
        };
        self.submit_library_edit(super::personal_tasks::LibraryEdit::Tags {
            target,
            tags,
        })?;
        self.interaction.report(
            "Tags saved · u undo · tags do not change folder membership".into(),
        );
        Ok(())
    }

    fn show_occurrences(&mut self) -> Result<()> {
        let article = self.current().context("Select an article")?.id;
        let choices = self
            .store
            .article_occurrences(article)?
            .into_iter()
            .map(|article| Choice {
                title: format!("{} · {}", article.feed, article.title),
                detail: format!(
                    "{}\n{}",
                    article.url,
                    "Open this occurrence to retain its feed/discussion links",
                ),
                action: ChoiceAction::JumpArticle(article.id),
            })
            .collect();
        self.inspect(choices)
    }

    fn toggle_pin(&mut self) -> Result<()> {
        ensure!(
            !matches!(self.nav.view(), View::Group(_)),
            "Expand the group before pinning a destination"
        );
        self.layout.toggle_pin(self.nav.view());
        self.save_navigation()
    }

    pub(super) fn toggle_navigation_group(
        &mut self,
        argument: &str,
    ) -> Result<()> {
        let group = match argument {
            "feeds" => "FEEDS",
            "servers" => "SERVERS",
            "lists" | "smart" => "SMART LISTS",
            "folders" => "YOUR FOLDERS",
            "pinned" => "PINNED",
            "" => {
                self.nav
                    .navigation()
                    .get(self.nav.nav_selected())
                    .context("Select a navigation section")?
                    .group
            }
            _ => bail!("Use :collapse feeds|servers|lists|folders|pinned"),
        };
        ensure!(
            group != "LIBRARY",
            "Built-in library destinations stay visible"
        );
        if self.layout.toggle_group(group) {
            self.nav.project_destination(View::Group(group.into()));
        }
        self.save_navigation_projection()
    }

    fn reorder_navigation(&mut self, forward: bool) -> Result<()> {
        ensure!(
            !matches!(self.nav.view(), View::Group(_)),
            "Select a destination to reorder"
        );
        self.layout.reorder_destination(
            self.nav.view(),
            self.nav.navigation_sections().destinations(),
            forward,
        )?;
        self.save_navigation()
    }

    fn save_navigation(&mut self) -> Result<()> {
        self.nav.customize(self.layout.workspace());
        self.save_navigation_projection()
    }

    fn save_navigation_projection(&mut self) -> Result<()> {
        self.save_reading_position()?;
        self.persist_preferences()?;
        let previous = self.nav.view().clone();
        self.project_navigation();
        if !previous.same_destination(self.nav.view())
            || matches!(self.nav.view(), View::Group(_))
        {
            self.filter()?;
        }
        Ok(())
    }
}

fn bulk_edit(
    argument: &str,
    library: &crate::model::Library,
) -> Result<BulkEdit> {
    let (verb, value) = argument.split_once(' ').unwrap_or((argument, ""));
    Ok(match verb {
        "read" => BulkEdit::Unread(false),
        "unread" => BulkEdit::Unread(true),
        "favorite" => BulkEdit::Favorite(true),
        "unfavorite" => BulkEdit::Favorite(false),
        "queue" => BulkEdit::Queue(true),
        "unqueue" => BulkEdit::Queue(false),
        "tag" => BulkEdit::Tag(value.into()),
        "folder" => {
            let folder = library
                .collections
                .iter()
                .find(|folder| folder.name.eq_ignore_ascii_case(value))
                .context("Create/select an existing folder first")?;
            BulkEdit::Folder(folder.id)
        }
        _ => bail!(
            "Use :bulk read|unread|favorite|unfavorite|queue|unqueue|tag NAME|folder NAME"
        ),
    })
}
