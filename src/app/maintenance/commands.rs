use anyhow::{Result, bail, ensure};

use super::{App, Request, RetentionAction};
use crate::{
    app::{Action, ChoiceAction, View},
    model::OfflineLimits,
    storage::Cleanup,
};

impl App {
    pub(in crate::app) fn maintenance_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        match verb {
            "offline" => self.offline_command(argument)?,
            "storage" => self.storage_command(argument)?,
            "session" => self.reading_session_command(argument)?,
            "retention" => self.retention_command(argument)?,
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    fn offline_command(&mut self, argument: &str) -> Result<()> {
        let mut words = argument.split_whitespace();
        let action = words.next().unwrap_or("status");
        let size = words.next();
        ensure!(
            words.next().is_none(),
            "Use :offline prepare [MiB]|queue [MiB]|resume|cancel|release|status"
        );
        if action == "cancel" {
            ensure!(size.is_none(), "Cancellation accepts no size argument");
            self.cancel_maintenance();
            return Ok(());
        }
        let request = match action {
            "prepare" | "queue" => self.offline_request(action, size)?,
            "resume" => {
                ensure!(
                    self.network_enabled,
                    "Offline preparation requires an online session"
                );
                Request::Resume
            }
            "release" => Request::Release,
            "status" => Request::OfflineStatus,
            _ => bail!(
                "Use :offline prepare [MiB]|queue [MiB]|resume|cancel|release|status"
            ),
        };
        ensure!(
            size.is_none() || matches!(action, "prepare" | "queue"),
            "This offline action accepts no size argument"
        );
        self.submit_maintenance(request)
    }

    fn offline_request(
        &self,
        action: &str,
        size: Option<&str>,
    ) -> Result<Request> {
        ensure!(
            self.network_enabled,
            "Offline preparation requires an online session"
        );
        let mut limits = OfflineLimits::default();
        if let Some(size) = size {
            limits.bytes = size
                .parse::<u64>()?
                .checked_mul(1_048_576)
                .ok_or_else(|| anyhow::anyhow!("Invalid offline byte limit"))?;
        }
        limits.validate()?;
        Ok(Request::Offline {
            view: if action == "queue" {
                View::Queue
            } else {
                self.view.clone()
            },
            limits,
        })
    }

    fn cancel_maintenance(&mut self) {
        if let Some(worker) = &self.maintenance {
            worker
                .cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.status = concat!(
            "Cancellation requested · ",
            "the active HTTP request or transaction may finish first"
        )
        .into();
    }

    fn storage_command(&mut self, argument: &str) -> Result<()> {
        if argument == "cancel" {
            self.cancel_maintenance();
            return Ok(());
        }
        if argument.is_empty() || argument == "status" {
            return self.submit_maintenance(Request::Storage);
        }
        let scope = match argument {
            "clean images" => Cleanup::Images,
            "clean articles" => Cleanup::Articles,
            "clean all" => Cleanup::All,
            _ => bail!(
                "Use :storage [status|clean images|clean articles|clean all]"
            ),
        };
        self.show_document(
            "Confirm cache cleanup · Enter confirms · Esc cancels",
            &format!(
                r#"Scope: {scope:?}

Only recoverable caches are eligible.
Favorites, queued articles, folders, notes/highlights, the offline
package and remaining reading-session articles are protected.

Feed text and personal records stay intact.
SQLite can reuse freed pages without shrinking its database file.
Completed batches remain committed if cancellation/failure follows.
"#
            ),
            Some(ChoiceAction::CleanCache(scope)),
        );
        Ok(())
    }

    fn reading_session_command(&mut self, argument: &str) -> Result<()> {
        let request = match argument {
            "" | "status" => Request::Session { advance: false },
            "next" => Request::Session { advance: true },
            "end" => Request::EndSession,
            value => Request::Plan(value.parse::<u16>()?),
        };
        self.submit_maintenance(request)
    }

    pub(in crate::app) fn retention_command(
        &mut self,
        argument: &str,
    ) -> Result<()> {
        if argument == "cancel" {
            self.cancel_maintenance();
            return Ok(());
        }
        let action = match argument {
            "" | "status" => RetentionAction::Status,
            "preview" => RetentionAction::Preview,
            "apply" => RetentionAction::Apply,
            _ => bail!("Use :retention status|preview|apply|cancel"),
        };
        self.submit_maintenance(Request::Retention {
            action,
            article: self.current().map(|article| article.id),
        })
    }
}
