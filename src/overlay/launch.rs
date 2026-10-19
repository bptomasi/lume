//! Capture the current terminal rather than an older ancestor's PTY geometry.

use std::{
    path::Path,
    process::{Command, Stdio},
};

use super::{Configuration, Output};

pub(super) fn command(
    configuration: &Configuration,
    directory: &Path,
) -> Command {
    let mut command = isolated_command(configuration, directory);
    let output = match configuration.output {
        Output::X11 => "x11",
        Output::Wayland => "wayland",
    };
    command.args(["layer", "--silent", "--no-cache", "--output", output]);
    command.env("UEBERZUGPP_TMPDIR", directory);
    command.env("XDG_CONFIG_HOME", directory);
    command.env("XDG_CACHE_HOME", directory);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    command
}

fn isolated_command(
    configuration: &Configuration,
    directory: &Path,
) -> Command {
    if let Some(isolation) = &configuration.isolation {
        let mut command = Command::new(isolation);
        // The read-only root is nodev. Preserve the existing PTY mount so the
        // helper can inspect its own terminal; only its runtime permits writes.
        command.args([
            "--die-with-parent",
            "--unshare-pid",
            "--ro-bind",
            "/",
            "/",
            "--dev-bind",
            "/dev/pts",
            "/dev/pts",
            "--proc",
            "/proc",
            "--bind",
        ]);
        command.arg(directory).arg(directory).arg("--");
        command.arg(&configuration.executable);
        command
    } else {
        Command::new(&configuration.executable)
    }
}
