//! Track the active Git identity without perpetual rebuilds from absent refs.
use anyhow::{Result, ensure};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub(super) struct Metadata {
    pub(super) revision: String,
    pub(super) watched: Vec<PathBuf>,
}

pub(super) fn inspect(
    directory: &Path,
    supplied: Option<&str>,
) -> Result<Metadata> {
    let revision = supplied
        .map(str::to_owned)
        .or_else(|| git(directory, &["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "unknown".into());
    ensure!(
        revision == "unknown"
            || (revision.len() == 40
                && revision.bytes().all(|byte| byte.is_ascii_hexdigit())),
        "LUME_BUILD_REVISION must be a full Git revision"
    );
    let watched = if supplied.is_some() {
        Vec::new()
    } else {
        watch_paths(directory)
    };
    Ok(Metadata { revision, watched })
}

fn watch_paths(directory: &Path) -> Vec<PathBuf> {
    let Some(head) = git_path(directory, "HEAD").filter(|path| path.is_file())
    else {
        return Vec::new();
    };
    let mut watched = vec![head.clone()];
    let content = std::fs::read_to_string(head).unwrap_or_default();
    let Some(reference) = content.trim().strip_prefix("ref: ") else {
        // A detached checkout stores its revision directly in HEAD.
        return watched;
    };
    let Some(path) = git_path(directory, reference) else {
        return watched;
    };
    if path.is_file() {
        watched.push(path);
    } else {
        // Packed/unborn refs can later become loose files. Watch their nearest
        // existing parent so creation is observed without watching absent files.
        if let Some(parent) =
            path.ancestors().skip(1).find(|path| path.is_dir())
        {
            watched.push(parent.to_owned());
        }
        if let Some(packed) =
            git_path(directory, "packed-refs").filter(|path| path.is_file())
        {
            watched.push(packed);
        }
    }
    watched
}

fn git_path(directory: &Path, name: &str) -> Option<PathBuf> {
    git(directory, &["rev-parse", "--git-path", name])
        .map(|path| directory.join(path))
}

fn git(directory: &Path, arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(directory)
        .args(arguments)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
