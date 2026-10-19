//! Conservative backend hints never start a competing stdin capability reader.

use lume::{
    app::App,
    media::ImageProtocol,
    overlay::{Configuration, Output},
};

#[cfg(unix)]
use lume::overlay::GridOrigin;

pub(super) enum ConfigurationKind {
    #[cfg(unix)]
    X11 {
        display: String,
        window: u32,
        origin: GridOrigin,
    },
    Helper(Configuration),
}

pub(super) fn start(
    app: &mut App,
    path: &std::path::Path,
    offline: bool,
) -> Option<lume::overlay::Runtime> {
    if let Some(directory) = path.parent()
        && let Err(error) = app.enable_images(directory.join("images"), offline)
    {
        app.status = format!("Images unavailable: {error}");
    }
    let runtime = match configure(app) {
        #[cfg(unix)]
        Ok(ConfigurationKind::X11 {
            display,
            window,
            origin,
        }) => lume::overlay::Runtime::start_x11(display, window, origin),
        Ok(ConfigurationKind::Helper(configuration)) => {
            lume::overlay::Runtime::start(Ok(configuration))
        }
        Err(error) => lume::overlay::Runtime::start(Err(error)),
    };
    match runtime {
        Ok(runtime) => Some(runtime),
        Err(error) => {
            app.image_overlay_failed(&error.to_string());
            None
        }
    }
}

pub(super) fn configure(app: &mut App) -> Result<ConfigurationKind, String> {
    let term = std::env::var("TERM").unwrap_or_default();
    let program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    let multiplexed = std::env::var_os("TMUX").is_some()
        || std::env::var_os("ZELLIJ").is_some();
    let overlay = overlay_configuration();
    let mut protocol = detect(&term, &program, multiplexed);
    if protocol == ImageProtocol::Blocks && !multiplexed && overlay.is_ok() {
        protocol = ImageProtocol::Overlay;
    }
    app.configure_image_terminal(protocol, pixels());
    overlay
}

fn overlay_configuration() -> Result<ConfigurationKind, String> {
    let output = match std::env::var("LUME_OVERLAY_BACKEND").as_deref() {
        Ok("x11") => Output::X11,
        Ok("wayland") => Output::Wayland,
        Ok(_) => {
            return Err("LUME_OVERLAY_BACKEND must be x11 or wayland".into());
        }
        Err(_) => detected_output().ok_or(
            "Pixels need an X11 terminal or a supported Wayland compositor",
        )?,
    };
    #[cfg(unix)]
    if output == Output::X11 {
        let display = std::env::var("DISPLAY")
            .map_err(|_| "X11 images require DISPLAY")?;
        let value = std::env::var("WINDOWID")
            .map_err(|_| "X11 images require the terminal WINDOWID")?;
        return Ok(ConfigurationKind::X11 {
            display,
            window: window_id(&value)?,
            origin: grid_origin(
                std::env::var("LUME_IMAGE_PADDING").ok().as_deref(),
            )?,
        });
    }
    let executable = std::env::var_os("LUME_UEBERZUGPP")
        .map(std::path::PathBuf::from)
        .or_else(|| find_program("ueberzugpp"))
        .and_then(|path| path.canonicalize().ok())
        .ok_or("Install ueberzugpp 2.9.10 or set LUME_UEBERZUGPP")?;
    Ok(ConfigurationKind::Helper(Configuration {
        executable,
        output,
        isolation: None,
    }))
}

#[cfg(unix)]
fn window_id(value: &str) -> Result<u32, String> {
    let parsed = if let Some(hex) = value.strip_prefix("0x") {
        u32::from_str_radix(hex, 16)
    } else {
        value.parse()
    };
    parsed
        .ok()
        .filter(|id| *id != 0)
        .ok_or_else(|| "WINDOWID must identify the current X11 terminal".into())
}

#[cfg(unix)]
fn grid_origin(value: Option<&str>) -> Result<GridOrigin, String> {
    let Some(value) = value else {
        return Ok(GridOrigin::Fixed([0, 0]));
    };
    if value == "auto" {
        return Ok(GridOrigin::Centered);
    }
    let invalid = || "LUME_IMAGE_PADDING must be x,y pixels or auto".to_owned();
    let (x, y) = value.split_once(',').ok_or_else(invalid)?;
    let x = x.parse::<u16>().map_err(|_| invalid())?;
    let y = y.parse::<u16>().map_err(|_| invalid())?;
    if x > 4096 || y > 4096 {
        return Err(invalid());
    }
    Ok(GridOrigin::Fixed([x, y]))
}

fn find_program(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|path| path.join(name))
            .find(|path| path.is_file())
            .and_then(|path| path.canonicalize().ok())
    })
}

fn detected_output() -> Option<Output> {
    if std::env::var_os("DISPLAY").is_some()
        && std::env::var_os("WINDOWID").is_some()
    {
        return Some(Output::X11);
    }
    let supported = [
        "SWAYSOCK",
        "HYPRLAND_INSTANCE_SIGNATURE",
        "NIRI_SOCKET",
        "WAYFIRE_SOCKET",
    ]
    .iter()
    .any(|name| std::env::var_os(name).is_some());
    if supported && std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return Some(Output::Wayland);
    }
    if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("x11")
        && std::env::var_os("DISPLAY").is_some()
    {
        return Some(Output::X11);
    }
    None
}

pub(super) fn pixels() -> (u16, u16) {
    let Ok(size) = crossterm::terminal::window_size() else {
        return (10, 20);
    };
    if size.columns == 0 || size.rows == 0 {
        return (10, 20);
    }
    (size.width / size.columns, size.height / size.rows)
}

fn detect(term: &str, program: &str, multiplexed: bool) -> ImageProtocol {
    if multiplexed || term.contains("alacritty") {
        return ImageProtocol::Blocks;
    }
    if matches!(term, "xterm-kitty" | "xterm-ghostty")
        || matches!(program, "kitty" | "ghostty")
    {
        return ImageProtocol::Kitty;
    }
    if matches!(program, "iTerm.app" | "WezTerm") {
        return ImageProtocol::Iterm2;
    }
    if matches!(term, "foot" | "foot-extra") {
        return ImageProtocol::Sixel;
    }
    ImageProtocol::Blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn image_padding_distinguishes_fixed_and_dynamic_terminal_grids() {
        assert!(matches!(grid_origin(None), Ok(GridOrigin::Fixed([0, 0]))));
        assert!(matches!(
            grid_origin(Some("8,12")),
            Ok(GridOrigin::Fixed([8, 12]))
        ));
        assert!(matches!(
            grid_origin(Some("auto")),
            Ok(GridOrigin::Centered)
        ));
        for value in ["", "8", "8,12,4", "-1,0", "0,65536", "4097,0"] {
            assert!(grid_origin(Some(value)).is_err());
        }
    }

    #[test]
    #[cfg(unix)]
    fn window_identity_accepts_decimal_and_hex_but_rejects_missing_authority() {
        assert_eq!(window_id("42"), Ok(42));
        assert_eq!(window_id("0x2a"), Ok(42));
        for value in ["", "0", "-1", "0x", "4294967296"] {
            assert!(window_id(value).is_err());
        }
    }

    #[test]
    fn native_hints_do_not_send_graphics_to_unsupported_or_multiplexed_terminals()
     {
        for (term, program, expected) in [
            ("alacritty", "", ImageProtocol::Blocks),
            ("xterm-256color", "", ImageProtocol::Blocks),
            ("xterm-kitty", "", ImageProtocol::Kitty),
            ("xterm-ghostty", "ghostty", ImageProtocol::Kitty),
            ("xterm-256color", "WezTerm", ImageProtocol::Iterm2),
            ("xterm-256color", "iTerm.app", ImageProtocol::Iterm2),
            ("foot", "", ImageProtocol::Sixel),
        ] {
            assert_eq!(detect(term, program, false), expected);
            assert_eq!(detect(term, program, true), ImageProtocol::Blocks);
        }
    }
}
