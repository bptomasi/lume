//! Explicit connections expose credential sources and ordinary Library actions.

use anyhow::{Context, Result, bail, ensure};

use super::{Action, App, Choice, ChoiceAction, Focus, Mode, View};

mod token;
pub use token::ServerToken;

impl App {
    pub(super) fn server_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        match verb {
            "server" => return self.connect_server(argument).map(Some),
            "servers" => self.inspect_servers()?,
            "server-history" => {
                self.store.restart_server_history(self.selected_server()?)?;
                self.reload()?;
                self.status =
                    "History restarted · cached articles retained".into();
                return Ok(Some(Action::Refresh));
            }
            "unserver" => {
                self.remove_destination(View::Server(self.selected_server()?))?;
            }
            "server-enable" | "server-disable" => {
                return self.enable_server(verb == "server-enable").map(Some);
            }
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    fn connect_server(&mut self, argument: &str) -> Result<Action> {
        let fields = argument.split_whitespace().collect::<Vec<_>>();
        ensure!(fields.len() == 2, "Usage: :server URL TOKEN_ENV|--token");
        if fields[1] == "--token" {
            return self.begin_server_token(fields[0]);
        }
        let id = self.store.add_server(fields[0], fields[1])?;
        self.finish_server_connection(id)
    }

    fn finish_server_connection(&mut self, id: i64) -> Result<Action> {
        let view = View::Server(id);
        self.reveal_server(&view)?;
        self.view = view;
        self.reload()?;
        self.focus = Focus::Library;
        self.status = "Server added · collecting in the background".into();
        Ok(Action::Refresh)
    }

    pub(super) fn reveal_server(&mut self, view: &View) -> Result<()> {
        if !matches!(view, View::Server(_)) {
            return Ok(());
        }
        let group = if self
            .workspace
            .pins
            .iter()
            .any(|pinned| pinned.same_destination(view))
        {
            "PINNED"
        } else {
            "SERVERS"
        };
        if self
            .workspace
            .collapsed_groups
            .iter()
            .any(|item| item == group)
        {
            self.workspace.collapsed_groups.retain(|item| item != group);
            self.persist_preferences()?;
            self.project_navigation();
        }
        Ok(())
    }

    fn enable_server(&mut self, enabled: bool) -> Result<Action> {
        let id = self.selected_server()?;
        self.store.set_server_enabled(id, enabled)?;
        self.reload()?;
        self.status = if enabled {
            "Server enabled"
        } else {
            "Server paused · cached articles remain readable"
        }
        .into();
        Ok(if enabled {
            Action::Refresh
        } else {
            Action::None
        })
    }

    fn selected_server(&self) -> Result<i64> {
        match self.view {
            View::Server(id) => Ok(id),
            View::Feed(id) => self
                .library
                .feeds
                .iter()
                .find(|feed| feed.id == id)
                .and_then(|feed| feed.server.as_ref())
                .map(|server| server.server_id)
                .context("Select a server in the Library first"),
            _ => bail!("Select a server in the Library first"),
        }
    }

    fn inspect_servers(&mut self) -> Result<()> {
        ensure!(
            !self.library.servers.is_empty(),
            "No servers · :server URL TOKEN_ENV adds a connection"
        );
        let choices = self
            .library
            .servers
            .iter()
            .map(|server| {
                let mut choice = server_choice(server);
                for feed in self.library.feeds.iter().filter(|feed| {
                    feed.server
                        .as_ref()
                        .is_some_and(|remote| remote.server_id == server.id)
                }) {
                    if let Some(remote) = &feed.server {
                        choice.detail.push_str(&format!(
                            "\n{}: {}",
                            feed.title,
                            remote.sync_progress()
                        ));
                    }
                }
                choice
            })
            .collect();
        self.inspect(choices)
    }
}

fn server_choice(server: &crate::model::Server) -> Choice {
    let state = if server.enabled { "Enabled" } else { "Paused" };
    let error = if server.error.is_empty() {
        "No collection error recorded"
    } else {
        &server.error
    };
    Choice {
        title: server.url.clone(),
        detail: format!(
            "{state} · credential source: {}\n{error}\n\n\
             :server-enable / :server-disable · dd removes the connection",
            if server.token.is_some() {
                "TOML (hidden)"
            } else {
                server.token_env.as_str()
            }
        ),
        action: ChoiceAction::SetView(View::Server(server.id)),
    }
}

pub(super) fn destinations(
    library: &crate::model::Library,
) -> Vec<(View, String, &'static str)> {
    let mut destinations = Vec::new();
    for server in &library.servers {
        let label = server
            .url
            .strip_prefix("https://")
            .or_else(|| server.url.strip_prefix("http://"))
            .unwrap_or(&server.url);
        let state = if !server.enabled {
            "[paused] "
        } else if !server.error.is_empty() {
            "[error] "
        } else {
            ""
        };
        destinations.push((
            View::Server(server.id),
            format!("{state}{label}"),
            "SERVERS",
        ));
        for feed in &library.feeds {
            if !feed.server.as_ref().is_some_and(|remote| {
                remote.server_id == server.id && remote.present
            }) {
                continue;
            }
            destinations.push((
                View::Feed(feed.id),
                format!("  · {}", feed.title),
                "SERVERS",
            ));
        }
    }
    destinations
}
