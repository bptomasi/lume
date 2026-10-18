//! Local import/export dispatch; network discovery stays with the worker.

use std::path::Path;

use anyhow::Result;

use super::{Action, App, Input, InputKind};

impl App {
    pub(super) fn exchange_input(&mut self, input: &Input) -> Result<Action> {
        use crate::exchange::{markdown, opml, read_import};
        let path = Path::new(input.text.trim());
        match input.kind {
            InputKind::Discover => {
                let url = crate::source::feed_url(&input.text)?;
                return Ok(Action::Discover(url.to_string()));
            }
            InputKind::Import => {
                let candidates = opml::decode(&read_import(path)?)?;
                let count = self.store.import_feeds(&candidates)?;
                self.reload()?;
                self.status =
                    format!("Imported {count} subscriptions · r collect");
            }
            InputKind::ExportArticle(id) => {
                markdown::article(&self.store, id, path)?;
                self.status = "Article exported to Markdown".into();
            }
            InputKind::ExportNotebook => {
                markdown::notebook(&self.store, path)?;
                self.status = "Notebook exported to Markdown".into();
            }
            InputKind::ExportOpml => {
                opml::export(&self.store, path)?;
                self.status = "Subscriptions exported to OPML".into();
            }
            _ => {}
        }
        Ok(Action::None)
    }

    pub(super) fn exchange_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        let kind = match verb {
            "discover" => InputKind::Discover,
            "import" => InputKind::Import,
            "export" => match self.current() {
                Some(article) => InputKind::ExportArticle(article.id),
                None => anyhow::bail!("Select an article to export"),
            },
            "export-notebook" => InputKind::ExportNotebook,
            "export-opml" => InputKind::ExportOpml,
            _ => return Ok(None),
        };
        let input = Input {
            kind,
            text: argument.to_owned(),
            previous_search: String::new(),
            editor: super::editor::textarea(argument),
            completion: 0,
        };
        self.exchange_input(&input).map(Some)
    }
}

impl App {
    pub(super) fn begin_workspace_export(
        &mut self,
        command: super::WorkspaceCommand,
    ) {
        use super::WorkspaceCommand as C;
        let kind = match command {
            C::ExportArticle => {
                let Some(article) = self.current() else {
                    return;
                };
                InputKind::ExportArticle(article.id)
            }
            C::ExportNotebook => InputKind::ExportNotebook,
            C::ExportOpml => InputKind::ExportOpml,
            _ => return,
        };
        self.begin_input(kind, String::new());
    }
}
