use std::{path::Path, sync::atomic::AtomicBool};

use anyhow::{Context, Result, ensure};

use super::{Outcome, Request, RetentionAction, report};
use crate::{model::ReadingSession, store::Store};

pub(super) fn perform(
    path: &Path,
    request: Request,
    cancel: &AtomicBool,
    progress: impl Fn(String),
) -> Result<Outcome> {
    let store = Store::open(path)?;
    let images = path
        .parent()
        .context("Library has no parent directory")?
        .join("images");
    match request {
        Request::Retention { action, article } => {
            retention(&store, &images, action, article, cancel)
        }
        Request::Offline { view, limits } => {
            let state = crate::offline::capture(&store, view, limits)?;
            prepare(&store, &images, state, cancel, progress)
        }
        Request::Resume => {
            let state = store
                .offline_state()?
                .context("No offline package to resume")?;
            prepare(&store, &images, state, cancel, progress)
        }
        Request::Release => release(&store, &images),
        Request::OfflineStatus => Ok(Outcome::Offline(
            store
                .offline_state()?
                .context("No offline package; use :offline queue")?,
        )),
        Request::Storage(unit) => Ok(Outcome::Text(
            crate::storage::inspect_with_unit(&store, &images, unit)?,
        )),
        Request::Clean(scope) => Ok(Outcome::Text(crate::storage::clean(
            &store, &images, scope, cancel,
        )?)),
        Request::Plan(budget) => {
            Ok(Outcome::Plan(store.plan_reading_session(budget)?))
        }
        Request::Start(session) => start(&store, session),
        Request::Session { advance } => session(&store, advance),
        Request::EndSession => end_session(&store),
    }
}

fn retention(
    store: &Store,
    images: &Path,
    action: RetentionAction,
    article: Option<i64>,
    cancel: &AtomicBool,
) -> Result<Outcome> {
    let now = chrono::Utc::now().timestamp();
    let (report, preview) = match action {
        RetentionAction::Status => {
            return Ok(Outcome::RetentionStatus(crate::retention::status(
                store,
            )?));
        }
        RetentionAction::Preview => {
            (crate::retention::preview(store, images, now)?, true)
        }
        RetentionAction::Apply => (
            crate::retention::apply(store, images, now, cancel, article)?,
            false,
        ),
        RetentionAction::Automatic => {
            let report = crate::retention::automatic(
                store, images, now, cancel, article,
            )?;
            return Ok(Outcome::AutomaticRetention {
                report,
                delay: crate::retention::next_check(store, now)?,
            });
        }
    };
    Ok(Outcome::Retention { report, preview })
}

fn prepare(
    store: &Store,
    images: &Path,
    state: crate::model::OfflineState,
    cancel: &AtomicBool,
    progress: impl Fn(String),
) -> Result<Outcome> {
    Ok(Outcome::Offline(crate::offline::prepare(
        store,
        images,
        state,
        cancel,
        |state| progress(report::offline(state)),
    )?))
}

fn start(store: &Store, session: ReadingSession) -> Result<Outcome> {
    session.validate()?;
    let transaction = store.write_transaction()?;
    ensure!(
        !session.items.is_empty(),
        "No recommended article fits this budget; configure interests or increase the budget"
    );
    for item in &session.items {
        ensure!(
            !store.is_deleted(item.article)?
                && store.article(item.article)?.url == item.url,
            "A planned article changed; generate a new plan"
        );
    }
    store.save_reading_session(&session)?;
    transaction.commit()?;
    Ok(Outcome::Session(session))
}

fn session(store: &Store, advance: bool) -> Result<Outcome> {
    let transaction = store.write_transaction()?;
    let mut session = store
        .reading_session()?
        .context("No reading session; use :session 20")?;
    if advance {
        session.position = (session.position + 1).min(session.items.len());
    }
    while let Some(item) = session.items.get(session.position) {
        if !store.is_deleted(item.article)?
            && store.article(item.article)?.url == item.url
        {
            break;
        }
        session.position += 1;
    }
    if advance || session.position > 0 {
        store.save_reading_session(&session)?;
    }
    transaction.commit()?;
    Ok(Outcome::Session(session))
}

fn end_session(store: &Store) -> Result<Outcome> {
    store.end_reading_session()?;
    Ok(Outcome::Text(
        "Reading session ended; articles and personal data are unchanged."
            .into(),
    ))
}

fn release(store: &Store, images: &Path) -> Result<Outcome> {
    crate::offline::release(store, images)?;
    Ok(Outcome::Text(
        concat!(
            "Offline protection and pinned images released; ",
            "cached article text remains."
        )
        .into(),
    ))
}
