//! Controlled worker boundaries prove that navigation never waits for grammar work.

use std::{sync::mpsc, time::Duration};

use anyhow::Result;
use ratatui::text::Line;

use super::{CACHE_BUDGET, Cached, CodeKey, Completed, LiveRenderer};

#[test]
fn blocked_syntax_work_does_not_block_layout_navigation_or_drop() -> Result<()>
{
    let (release, permission) = mpsc::channel();
    let (started, starts) = mpsc::channel();
    let mut renderer = LiveRenderer::spawn(move |text, _| {
        let _ = started.send(());
        if permission.recv_timeout(Duration::from_secs(3)).is_err() {
            return Vec::new();
        }
        vec![Line::raw(text.trim_end_matches('\n').to_owned())]
    })?;
    let first = renderer.render("```rust\nlet first = 1;\n```", 60);
    starts.recv_timeout(Duration::from_secs(3))?;

    let second = renderer.render("```go\nvar second = 2\n```", 60);

    assert!(first.text.contains("let first = 1;"));
    assert!(second.text.contains("var second = 2"));
    assert!(!second.text.contains("first"));
    assert!(renderer.is_pending());
    drop(renderer);
    release.send(())?;
    Ok(())
}

#[test]
fn cache_eviction_and_oversized_results_keep_the_retained_budget() -> Result<()>
{
    let mut renderer = LiveRenderer::spawn(|_, _| Vec::new())?;
    renderer.cache.push_back(Cached {
        completed: Completed {
            key: CodeKey {
                text: "old".into(),
                language: "rust".into(),
            },
            lines: Vec::new().into(),
        },
        bytes: CACHE_BUDGET,
    });
    renderer.retained_bytes = CACHE_BUDGET;

    renderer.retain(Completed {
        key: CodeKey {
            text: "new".into(),
            language: "go".into(),
        },
        lines: vec![Line::raw("new")].into(),
    });
    assert_eq!(renderer.cache.len(), 1);
    assert_eq!(renderer.cache[0].completed.key.text.as_ref(), "new");
    assert!(renderer.retained_bytes <= CACHE_BUDGET);
    renderer.retain(Completed {
        key: CodeKey {
            text: "huge".into(),
            language: "sql".into(),
        },
        lines: vec![Line::raw("x".repeat(CACHE_BUDGET + 1))].into(),
    });
    assert_eq!(renderer.cache.len(), 1);
    assert_eq!(renderer.cache[0].completed.key.text.as_ref(), "new");
    Ok(())
}

#[test]
fn disconnected_worker_clears_pending_and_preserves_plain_text() -> Result<()> {
    let (requests, receiver) = mpsc::sync_channel(1);
    let (sender, results) = mpsc::sync_channel(1);
    let mut renderer = LiveRenderer::spawn(|_, _| Vec::new())?;
    renderer.requests = requests;
    renderer.results = results;
    let before = renderer.render("```sql\nSELECT 42;\n```", 60);
    assert!(renderer.is_pending());

    drop(receiver);
    drop(sender);
    assert!(renderer.poll());
    let after = renderer.render("```sql\nSELECT 42;\n```", 60);

    assert!(renderer.is_stopped());
    assert!(!renderer.is_pending());
    assert_eq!(before.text, after.text);
    assert!(after.text.contains("SELECT 42;"));
    assert!(!renderer.poll());
    Ok(())
}

#[test]
fn revisiting_an_inflight_block_does_not_duplicate_work() -> Result<()> {
    let (requests, receiver) = mpsc::sync_channel(1);
    let mut renderer = LiveRenderer::spawn(|_, _| Vec::new())?;
    renderer.requests = requests;
    renderer.render("```rust\nlet value = 42;\n```", 60);
    let first = receiver.try_recv()?;

    renderer.render("A different article.", 60);
    renderer.render("```rust\nlet value = 42;\n```", 30);

    assert!(matches!(
        receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert_eq!(renderer.pending.len(), 1);
    assert_eq!(first.text.as_ref(), "let value = 42;\n");
    Ok(())
}
