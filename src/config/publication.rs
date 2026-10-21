//! Same-directory atomic publication; native locks serialize Lume writers.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use anyhow::{Context, Result, ensure};

pub(crate) fn prepare_directory(path: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    Ok(())
}

pub(crate) fn lock(path: &Path) -> Result<File> {
    let lock = path.with_file_name(format!(
        "{}.lock",
        path.file_name()
            .context("No configuration filename")?
            .to_string_lossy()
    ));
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(lock)?;
    file.try_lock().context(
        "Configuration is being updated by another Lume process; retry",
    )?;
    Ok(file)
}

pub(crate) fn read(path: &Path) -> Result<Option<String>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
    let mut text = String::new();
    file.take(2 * 1024 * 1024 + 1).read_to_string(&mut text)?;
    ensure!(text.len() <= 2 * 1024 * 1024, "Configuration exceeds 2 MiB");
    Ok(Some(text))
}

pub(crate) fn publish(
    path: &Path,
    previous: Option<&str>,
    text: &str,
) -> Result<()> {
    if previous == Some(text) {
        return Ok(());
    }
    let parent = path.parent().context("Configuration has no parent")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    if let Ok(metadata) = path.metadata() {
        ensure!(
            !metadata.permissions().readonly(),
            "Configuration is read-only"
        );
        temporary
            .as_file()
            .set_permissions(metadata.permissions())?;
    }
    temporary.write_all(text.as_bytes())?;
    temporary.as_file().sync_all()?;
    ensure!(
        read(path)?.as_deref() == previous,
        "Configuration changed during publication; retry without overwriting it"
    );
    temporary
        .persist(path)
        .context("Could not publish configuration")?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}
