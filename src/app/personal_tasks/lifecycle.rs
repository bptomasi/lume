//! Blocking shutdown acknowledgements preserve ticket identity and failures.

use super::*;
use std::time::{Duration, Instant};

fn writer(tickets: &[u64]) -> (PersonalWrites, mpsc::SyncSender<Completion>) {
    let (requests, _receiver) = mpsc::sync_channel(1);
    let (sender, results) = mpsc::sync_channel(1);
    let writer = PersonalWrites {
        requests,
        results,
        next: tickets.last().copied().unwrap_or(0),
        pending: RefCell::new(tickets.iter().copied().collect()),
        diagnostics: false,
    };
    (writer, sender)
}

fn completion(ticket: u64) -> Completion {
    Completion {
        ticket,
        result: Ok(Outcome::Checkpoint),
        started: None,
    }
}

#[test]
fn waits_drain_a_bounded_result_queue_in_ticket_order() -> Result<()> {
    let (writer, sender) = writer(&[1, 2, 3]);
    sender.send(completion(1))?;
    let producer = std::thread::spawn(move || -> Result<()> {
        sender.send(Completion {
            result: Err(anyhow!("Captured transaction failed")),
            ..completion(2)
        })?;
        sender.send(completion(3))?;
        Ok(())
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    for ticket in [1, 2, 3] {
        let acknowledged = writer.wait(deadline)?;
        assert_eq!(acknowledged.ticket, ticket);
        assert_eq!(acknowledged.result.is_err(), ticket == 2);
    }
    assert!(!writer.pending());
    producer
        .join()
        .map_err(|_| anyhow!("Result producer stopped"))??;
    Ok(())
}

#[test]
fn a_wait_timeout_retains_the_ticket_for_later_acknowledgement() -> Result<()> {
    let (writer, sender) = writer(&[7]);
    assert!(writer.wait(Instant::now()).is_err());
    assert!(writer.pending());
    sender.send(completion(7))?;
    assert_eq!(writer.wait(Instant::now())?.ticket, 7);
    assert!(!writer.pending());
    Ok(())
}

#[test]
fn a_disconnected_wait_acknowledges_each_ticket_as_a_failure() -> Result<()> {
    let (writer, sender) = writer(&[3, 4]);
    drop(sender);
    for ticket in [3, 4] {
        let acknowledged = writer.wait(Instant::now())?;
        assert_eq!(acknowledged.ticket, ticket);
        assert!(acknowledged.result.is_err());
    }
    assert!(!writer.pending());
    assert!(writer.wait(Instant::now()).is_err());
    Ok(())
}

fn app_after_failed_flush() -> Result<(
    crate::app::App,
    mpsc::SyncSender<Completion>,
    tempfile::TempDir,
)> {
    let directory = tempfile::tempdir()?;
    let store = crate::store::Store::open(&directory.path().join("notes.db"))?;
    let mut app = crate::app::App::new(store, false)?;
    let (writer, sender) = writer(&[1, 2]);
    app.personal =
        crate::app::personal_state::PersonalState::background(writer);
    sender.send(Completion {
        result: Err(anyhow!("Captured flag mutation failed")),
        ..completion(1)
    })?;
    assert!(app.flush_note_tasks_before(Instant::now()).is_err());
    assert!(app.notes_pending());
    Ok((app, sender, directory))
}

#[test]
fn a_flush_retry_reports_a_consumed_non_draft_failure() -> Result<()> {
    let (mut app, sender, _directory) = app_after_failed_flush()?;
    sender.send(completion(2))?;
    let error = app
        .flush_note_tasks_before(Instant::now())
        .err()
        .context("Prior mutation failure was lost")?;
    assert_eq!(error.to_string(), "Captured flag mutation failed");
    assert!(!app.notes_pending());
    app.flush_note_tasks_before(Instant::now())?;
    assert!(app.poll_note_tasks()?.is_none());
    Ok(())
}

#[test]
fn ordinary_poll_reports_a_retained_non_draft_failure_once() -> Result<()> {
    let (mut app, sender, _directory) = app_after_failed_flush()?;
    let error = app
        .poll_note_tasks()
        .err()
        .context("Missing retained mutation failure")?;
    assert_eq!(error.to_string(), "Captured flag mutation failed");
    assert!(app.notes_pending());
    assert!(app.poll_note_tasks()?.is_none());
    sender.send(completion(2))?;
    assert!(app.poll_note_tasks()?.is_some());
    assert!(!app.notes_pending());
    app.flush_note_tasks_before(Instant::now())?;
    Ok(())
}
