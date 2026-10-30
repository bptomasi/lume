//! Build metadata follows attached, detached, packed and worktree Git heads.
#[path = "../build_support/revision.rs"]
mod revision;

use anyhow::{Context, Result, ensure};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn git(directory: &Path, arguments: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(directory)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgSign=false",
            "-c",
            "user.name=Lume fixture",
            "-c",
            "user.email=lume@example.test",
        ])
        .args(arguments)
        .output()?;
    ensure!(
        output.status.success(),
        "Fixture Git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().into())
}

fn repository() -> Result<tempfile::TempDir> {
    let directory = tempfile::tempdir()?;
    git(
        directory.path(),
        &[
            "init",
            "-q",
            "--object-format=sha1",
            "--initial-branch=main",
        ],
    )?;
    git(
        directory.path(),
        &["commit", "--allow-empty", "-qm", "Fixture"],
    )?;
    Ok(directory)
}

fn git_path(directory: &Path, name: &str) -> Result<PathBuf> {
    Ok(directory.join(git(directory, &["rev-parse", "--git-path", name])?))
}

#[test]
fn only_the_actual_branch_is_watched_and_detached_heads_need_no_branch_ref()
-> Result<()> {
    let directory = repository()?;
    let root = directory.path();
    git(root, &["checkout", "-qb", "review"])?;
    let metadata = revision::inspect(root, None)?;
    assert_eq!(metadata.revision, git(root, &["rev-parse", "HEAD"])?);
    assert!(
        metadata
            .watched
            .contains(&git_path(root, "refs/heads/review")?)
    );
    assert!(
        !metadata
            .watched
            .contains(&git_path(root, "refs/heads/main")?)
    );
    git(root, &["checkout", "-q", "--detach"])?;
    git(root, &["update-ref", "-d", "refs/heads/main"])?;
    assert!(!git_path(root, "refs/heads/main")?.exists());
    let detached = revision::inspect(root, None)?;
    assert_eq!(detached.revision, metadata.revision);
    assert_eq!(detached.watched, [git_path(root, "HEAD")?]);
    assert!(detached.watched.iter().all(|path| path.exists()));
    Ok(())
}

#[test]
fn packed_and_unborn_refs_watch_existing_parents_and_worktrees_follow_their_head()
-> Result<()> {
    let directory = repository()?;
    let root = directory.path();
    git(root, &["pack-refs", "--all"])?;
    let metadata = revision::inspect(root, None)?;
    assert!(metadata.watched.contains(&git_path(root, "packed-refs")?));
    assert!(metadata.watched.iter().all(|path| path.exists()));
    let worktree = root.join("linked");
    git(
        root,
        &[
            "worktree",
            "add",
            "--detach",
            worktree.to_str().context("UTF-8 fixture path")?,
            "HEAD",
        ],
    )?;
    let linked = revision::inspect(&worktree, None)?;
    assert_eq!(linked.revision, metadata.revision);
    assert_eq!(linked.watched, [git_path(&worktree, "HEAD")?]);
    let unborn = tempfile::tempdir()?;
    git(unborn.path(), &["init", "-q", "--initial-branch=main"])?;
    let pending = revision::inspect(unborn.path(), None)?;
    assert_eq!(pending.revision, "unknown");
    assert!(pending.watched.iter().all(|path| path.exists()));
    assert!(pending.watched.iter().any(|path| path.is_dir()));
    Ok(())
}

#[test]
fn source_archives_and_explicit_revision_validation_do_not_require_git_metadata()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let unknown = revision::inspect(directory.path(), None)?;
    assert_eq!(unknown.revision, "unknown");
    assert!(unknown.watched.is_empty());
    let sha = "a".repeat(40);
    let supplied = revision::inspect(directory.path(), Some(&sha))?;
    assert_eq!(supplied.revision, sha);
    assert!(supplied.watched.is_empty());
    for invalid in ["short", "not a valid revision", &"x".repeat(40)] {
        assert!(revision::inspect(directory.path(), Some(invalid)).is_err());
    }
    Ok(())
}
