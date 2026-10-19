//! Incremental note files preserve wikilinks and never overwrite local edits.
//!
//! Article and note exports share atomic publication and the directory lock.
//! Separate manifests keep their ownership independent. Removed notes leave
//! files intact; permanent note IDs cannot later alias a different annotation.

use super::incremental::{ExportLock, ExportReport, reconcile};
use crate::store::Store;
use anyhow::{Context, Result, ensure};
use std::path::Path;

/// Export notes as stable Note-ID.md files with source citations and metadata.
///
/// # Errors
/// Returns database/filesystem failures or a busy directory. Local edits become
/// conflicts, and neither missing notes nor Trash delete an exported file.
pub fn notebook(store: &Store, directory: &Path) -> Result<ExportReport> {
    std::fs::create_dir_all(directory)?;
    let directory = directory.canonicalize()?;
    let namespace = directory
        .to_str()
        .context("Export directory must be UTF-8")?;
    let _lock = ExportLock::acquire(&directory)?;
    let mut report = ExportReport::default();
    let mut offset = 0;
    loop {
        let page = store.search_notes("", None, offset, 100)?;
        if page.notes.is_empty() {
            return Ok(report);
        }
        offset += page.notes.len();
        for summary in page.notes {
            let note = store.note_export(summary.id)?;
            let content = render(&note)?;
            let path = directory.join(format!("Note-{}.md", summary.id));
            let previous = store.exported_note(namespace, summary.id)?;
            if reconcile(
                &path,
                previous.as_deref(),
                &content,
                &directory,
                &mut report,
            )? {
                store.record_note_export(namespace, summary.id, &content)?;
            }
        }
    }
}

fn render(detail: &crate::model::NoteExport) -> Result<String> {
    let note = &detail.note;
    let status = if note.metadata.resolved {
        "resolved"
    } else {
        "open"
    };
    let mut text = format!(
        "---\nid: Note-{}\ntype: {}\nstatus: {status}\npinned: {}\ntags: {}\nsource: {}\ncreated: {}\nupdated: {}\n---\n\n# Note {}\n\nSource: {}\n\n",
        note.comment.id,
        note.metadata.kind.name(),
        note.metadata.pinned,
        serde_json::to_string(&note.tags)?,
        serde_json::to_string(&detail.url)?,
        note.comment.created,
        note.metadata.updated_at,
        note.comment.id,
        detail.url,
    );
    if !note.comment.quote.is_empty() {
        for line in note.comment.quote.lines() {
            text.push_str(&format!("> {line}\n"));
        }
        text.push('\n');
    }
    text.push_str(&note.comment.body);
    text.push_str("\n\n## Backlinks\n\n");
    for related in &detail.backlinks {
        text.push_str(&format!("- [[Note-{}]]\n", related.id));
    }
    ensure!(text.len() <= 131072, "Note export exceeds 128 KiB");
    Ok(text)
}
