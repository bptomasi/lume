//! Incremental, owned Markdown reconciliation; user edits become conflicts.
//!
//! Each article has a stable filename and exact previous content in SQLite.
//! A cooperative directory lock serializes Lume exports. A crash after rename
//! but before manifest commit is recoverable when the next desired bytes match.
//! Removed/trashed notes never cause automatic filesystem deletion.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};

use crate::{
    model::{ArticleQuery, View},
    store::Store,
};

/// Outcome of one reconciliation; conflicting files remain untouched.
#[derive(Default)]
pub struct ExportReport {
    /// Newly created or updated owned files.
    pub written: usize,
    /// Files already equal to the desired export.
    pub unchanged: usize,
    /// Existing unowned or locally edited files, preserved exactly.
    pub conflicts: Vec<PathBuf>,
}

/// Reconcile annotated articles into stable `Lume-ID.md` files in a directory.
///
/// # Errors
/// Returns filesystem/SQLite failures, a busy export lock or oversized content.
/// Previous files remain complete; conflicts are returned without overwriting.
pub fn notebook(store: &Store, directory: &Path) -> Result<ExportReport> {
    fs::create_dir_all(directory)?;
    let directory = directory.canonicalize()?;
    let namespace = directory
        .to_str()
        .context("Export directory must be UTF-8")?;
    let _lock = ExportLock::acquire(&directory)?;
    let mut report = ExportReport::default();
    let mut offset = 0;
    loop {
        let page = store.query_articles(&ArticleQuery {
            view: View::Annotated,
            text: String::new(),
            offset,
            limit: 100,
            retained: None,
        })?;
        if page.articles.is_empty() {
            break;
        }
        offset += page.articles.len();
        for article in page.articles {
            let path = directory.join(format!("Lume-{}.md", article.id));
            let content = render(store, article.id)?;
            let previous = store.exported_content(namespace, article.id)?;
            if reconcile(
                &path,
                previous.as_deref(),
                &content,
                &directory,
                &mut report,
            )? {
                store.record_export(namespace, article.id, &content)?;
            }
        }
    }
    Ok(report)
}

fn render(store: &Store, article: i64) -> Result<String> {
    // Fixed-size Cursor fails before a managed export can exceed 1 MiB.
    let mut bytes = vec![0_u8; 1024 * 1024];
    let length = {
        let mut writer = std::io::Cursor::new(bytes.as_mut_slice());
        super::markdown::write_article(store, article, &mut writer, false)?;
        writer.position() as usize
    };
    bytes.truncate(length);
    Ok(String::from_utf8(bytes)?)
}

pub(super) fn reconcile(
    path: &Path,
    previous: Option<&str>,
    content: &str,
    directory: &Path,
    report: &mut ExportReport,
) -> Result<bool> {
    let initial = read_destination(path)?;
    let current = match &initial {
        Destination::Missing => None,
        Destination::Text(text) => Some(text.as_str()),
        Destination::Conflict => {
            report.conflicts.push(path.to_owned());
            return Ok(false);
        }
    };
    if current == Some(content) {
        report.unchanged += 1;
        return Ok(true);
    }
    if current.is_some() && current != previous {
        report.conflicts.push(path.to_owned());
        return Ok(false);
    }
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    temporary.write_all(content.as_bytes())?;
    temporary.as_file().sync_all()?;
    // Recheck immediately before rename; cooperative exporters hold the lock.
    ensure!(
        read_destination(path)? == initial,
        "Export changed while writing; retry after checking the destination"
    );
    if current.is_some() {
        temporary.persist(path)?;
    } else {
        temporary.persist_noclobber(path)?;
    }
    report.written += 1;
    Ok(true)
}

#[derive(PartialEq, Eq)]
enum Destination {
    Missing,
    Text(String),
    Conflict,
}

fn read_destination(path: &Path) -> Result<Destination> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Destination::Missing);
        }
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > 1024 * 1024
    {
        return Ok(Destination::Conflict);
    }
    Ok(match String::from_utf8(fs::read(path)?) {
        Ok(text) => Destination::Text(text),
        Err(_) => Destination::Conflict,
    })
}

pub(super) struct ExportLock(PathBuf);

impl ExportLock {
    pub(super) fn acquire(directory: &Path) -> Result<Self> {
        let path = directory.join(".lume-export.lock");
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .context(
                "Export already running; inspect .lume-export.lock \
                if a previous process crashed",
            )?;
        Ok(Self(path))
    }
}

impl Drop for ExportLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
