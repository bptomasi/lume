//! Library paths are independent of the executable and working directories.
//!
//! Explicit file paths are handled by the CLI. Directory flags override
//! LUME_DATA_DIR; HOME supplies only the default. Legacy locations are read
//! by the installer for migration, never discovered from the runtime cwd.

use std::{
    ffi::OsString,
    io::{Error, ErrorKind, Result},
    path::PathBuf,
};

pub(crate) fn database_path(
    directory: Option<PathBuf>,
    demo: bool,
) -> Result<PathBuf> {
    resolve(
        directory,
        std::env::var_os("LUME_DATA_DIR"),
        std::env::var_os("HOME"),
        demo,
    )
}

fn resolve(
    directory: Option<PathBuf>,
    configured: Option<OsString>,
    home: Option<OsString>,
    demo: bool,
) -> Result<PathBuf> {
    let directory = if let Some(directory) = directory {
        directory
    } else if let Some(configured) = configured {
        PathBuf::from(configured)
    } else {
        let home = home.filter(|home| !home.is_empty()).ok_or_else(|| {
            Error::new(
                ErrorKind::NotFound,
                "Set HOME, LUME_DATA_DIR, --data-dir or --database",
            )
        })?;
        PathBuf::from(home).join(".local/lume")
    };
    if directory.as_os_str().is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "The data directory must not be empty",
        ));
    }
    let name = if demo {
        "demo.sqlite3"
    } else {
        "library.sqlite3"
    };
    Ok(directory.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_library_and_demo_have_separate_files_under_home() -> Result<()> {
        let home = Some(OsString::from("/users/reader"));
        assert_eq!(
            resolve(None, None, home.clone(), false)?,
            PathBuf::from("/users/reader/.local/lume/library.sqlite3")
        );
        assert_eq!(
            resolve(None, None, home, true)?,
            PathBuf::from("/users/reader/.local/lume/demo.sqlite3")
        );
        Ok(())
    }

    #[test]
    fn environment_overrides_home_and_works_without_home() -> Result<()> {
        assert_eq!(
            resolve(None, Some("/data/study".into()), None, false)?,
            PathBuf::from("/data/study/library.sqlite3")
        );
        Ok(())
    }

    #[test]
    fn flag_overrides_environment_including_an_empty_environment() -> Result<()>
    {
        assert_eq!(
            resolve(Some("/chosen".into()), Some("".into()), None, false)?,
            PathBuf::from("/chosen/library.sqlite3")
        );
        Ok(())
    }

    #[test]
    fn missing_home_and_empty_directories_are_actionable_errors() {
        assert_eq!(
            resolve(None, None, None, false)
                .err()
                .map(|error| error.kind()),
            Some(ErrorKind::NotFound)
        );
        assert_eq!(
            resolve(None, Some("".into()), Some("/home".into()), false)
                .err()
                .map(|error| error.kind()),
            Some(ErrorKind::InvalidInput)
        );
    }
}
