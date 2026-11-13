//! Local import/export dispatch; network discovery stays with the worker.

use crate::app::commands::routing;

use std::path::Path;

use anyhow::Result;

use super::{Action, App, Input, InputKind};

impl App {
    pub(super) fn exchange_input(&mut self, input: &Input) -> Result<Action> {
        use crate::exchange::{markdown, opml, read_import};
        let path = Path::new(input.text.trim());
        match input.kind {
            InputKind::Discover => {
                let url = crate::urls::feed_url(&input.text)?;
                return Ok(Action::Discover(url.to_string()));
            }
            InputKind::Import => {
                let candidates = opml::decode(&read_import(path)?)?;
                let count = self.store.import_feeds(&candidates)?;
                self.reload()?;
                self.interaction.report(format!(
                    "Imported {count} subscriptions · r collect"
                ));
            }
            InputKind::ExportArticle(id) => {
                markdown::article(&self.store, id, path)?;
                self.interaction
                    .report("Article exported to Markdown".into());
            }
            InputKind::ExportNotebook => {
                markdown::notebook(&self.store, path)?;
                self.interaction
                    .report("Notebook exported to Markdown".into());
            }
            InputKind::ExportOpml => {
                opml::export(&self.store, path)?;
                self.interaction
                    .report("Subscriptions exported to OPML".into());
            }
            _ => {}
        }
        Ok(Action::None)
    }

    pub(super) fn exchange_command(
        &mut self,
        command: routing::ExchangeCommand,
        argument: &str,
    ) -> Result<Action> {
        let kind = match command {
            routing::ExchangeCommand::Discover => InputKind::Discover,
            routing::ExchangeCommand::Import => InputKind::Import,
            routing::ExchangeCommand::Export => match self.current() {
                Some(article) => InputKind::ExportArticle(article.id),
                None => anyhow::bail!("Select an article to export"),
            },
            routing::ExchangeCommand::ExportNotebook => {
                InputKind::ExportNotebook
            }
            routing::ExchangeCommand::ExportOpml => InputKind::ExportOpml,
        };
        let input = Input::new(kind, argument.to_owned(), String::new());
        self.exchange_input(&input)
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
