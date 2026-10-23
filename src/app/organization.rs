//! Captured bulk actions, independent tags and personal navigation preferences.

use std::collections::BTreeSet;

use anyhow::{Context, Result, bail, ensure};

use crate::model::{BulkEdit, TagTarget};

use super::{Action, App, Choice, ChoiceAction, View};

impl App {
    pub(super) fn organization_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        match verb {
            "select-article" => self.toggle_article_selection()?,
            "select-page" => self.select_page()?,
            "clear-selection" => self.marked.clear(),
            "bulk" => self.apply_bulk(argument)?,
            "tags" | "highlight-tags" => self.edit_tags(verb, argument)?,
            "related" => self.show_occurrences()?,
            "pin" => self.toggle_pin()?,
            "collapse" => self.toggle_navigation_group(argument)?,
            "move-up" | "move-down" => {
                self.reorder_navigation(verb == "move-down")?
            }
            "continue" => self.set_view(View::Continue)?,
            "since-last-visit" => {
                self.set_view(View::SinceLastVisit(self.workspace.last_visit))?
            }
            "grouped" => self.set_view(View::Grouped)?,
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    fn toggle_article_selection(&mut self) -> Result<()> {
        let article = self.current().context("Select an article")?.id;
        if !self.marked.remove(&article) {
            ensure!(
                self.marked.len() < 1000,
                "Selection limit is 1,000 articles"
            );
            self.marked.insert(article);
        }
        self.status = format!(
            "{} selected · :bulk ACTION · :clear-selection",
            self.marked.len()
        );
        Ok(())
    }

    fn select_page(&mut self) -> Result<()> {
        let selection: BTreeSet<_> = self
            .marked
            .iter()
            .copied()
            .chain(self.library.articles.iter().map(|article| article.id))
            .collect();
        ensure!(selection.len() <= 1000, "Selection limit is 1,000 articles");
        self.marked = selection;
        self.status =
            format!("{} captured articles selected", self.marked.len());
        Ok(())
    }

    fn apply_bulk(&mut self, argument: &str) -> Result<()> {
        let (verb, value) = argument.split_once(' ').unwrap_or((argument, ""));
        let edit = match verb {
            "read" => BulkEdit::Unread(false),
            "unread" => BulkEdit::Unread(true),
            "favorite" => BulkEdit::Favorite(true),
            "unfavorite" => BulkEdit::Favorite(false),
            "queue" => BulkEdit::Queue(true),
            "unqueue" => BulkEdit::Queue(false),
            "tag" => BulkEdit::Tag(value.into()),
            "folder" => {
                let folder = self
                    .library
                    .collections
                    .iter()
                    .find(|folder| folder.name.eq_ignore_ascii_case(value))
                    .context("Create/select an existing folder first")?;
                BulkEdit::Folder(folder.id)
            }
            _ => bail!(
                "Use :bulk read|unread|favorite|unfavorite|queue|unqueue|tag NAME|folder NAME"
            ),
        };
        self.submit_article_edit(self.marked.clone(), edit)?;
        let count = self.marked.len();
        self.marked.clear();
        self.retained_article = None;
        if self.personal_writes.is_none() {
            self.reload()?;
        }
        self.status = format!(
            "Updated {count} captured articles · u undoes the entire batch"
        );
        Ok(())
    }

    fn edit_tags(&mut self, verb: &str, argument: &str) -> Result<()> {
        let target = if verb == "highlight-tags" {
            TagTarget::Comment(
                self.comments
                    .get(self.comment_selected)
                    .context("Select a highlight/comment")?
                    .id,
            )
        } else {
            TagTarget::Article(self.current().context("Select an article")?.id)
        };
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
        self.status =
            "Tags saved · u undo · tags do not change folder membership".into();
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
            !matches!(self.view, View::Group(_)),
            "Expand the group before pinning a destination"
        );
        if let Some(position) = self
            .workspace
            .pins
            .iter()
            .position(|view| view.same_destination(&self.view))
        {
            self.workspace.pins.remove(position);
        } else {
            self.workspace.pins.push(self.view.clone());
        }
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
                self.navigation
                    .get(self.nav_selected)
                    .context("Select a navigation section")?
                    .group
            }
            _ => bail!("Use :collapse feeds|servers|lists|folders|pinned"),
        };
        if let Some(position) = self
            .workspace
            .collapsed_groups
            .iter()
            .position(|item| item == group)
        {
            self.workspace.collapsed_groups.remove(position);
        } else {
            ensure!(
                group != "LIBRARY",
                "Built-in library destinations stay visible"
            );
            self.workspace.collapsed_groups.push(group.into());
            self.view = View::Group(group.into());
        }
        self.save_navigation_projection()
    }

    fn reorder_navigation(&mut self, forward: bool) -> Result<()> {
        ensure!(
            !matches!(self.view, View::Group(_)),
            "Select a destination to reorder"
        );
        let order = if self
            .workspace
            .pins
            .iter()
            .any(|view| view.same_destination(&self.view))
        {
            &mut self.workspace.pins
        } else {
            self.workspace.navigation_order =
                self.navigation_sections.destinations();
            &mut self.workspace.navigation_order
        };
        let index = order
            .iter()
            .position(|view| view.same_destination(&self.view))
            .context("Select a destination to reorder")?;
        let next = index
            .saturating_add_signed(if forward { 1 } else { -1 })
            .min(order.len().saturating_sub(1));
        order.swap(index, next);
        self.save_navigation()
    }

    fn save_navigation(&mut self) -> Result<()> {
        self.navigation_sections
            .customize(&self.workspace, &self.library);
        self.save_navigation_projection()
    }

    fn save_navigation_projection(&mut self) -> Result<()> {
        self.save_reading_position()?;
        self.persist_preferences()?;
        let previous = self.view.clone();
        self.project_navigation();
        if !previous.same_destination(&self.view)
            || matches!(self.view, View::Group(_))
        {
            self.filter()?;
        }
        Ok(())
    }
}
