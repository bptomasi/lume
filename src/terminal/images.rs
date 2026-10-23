//! Conservative native image hints never compete for terminal input.

use lume::{app::App, media::ImageProtocol};

pub(super) fn start(app: &mut App, path: &std::path::Path, offline: bool) {
    if let Some(directory) = path.parent()
        && let Err(error) = app.enable_images(directory.join("images"), offline)
    {
        app.status = format!("Images unavailable: {error}");
    }
    let term = std::env::var("TERM").unwrap_or_default();
    let program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    let multiplexed = std::env::var_os("TMUX").is_some()
        || std::env::var_os("ZELLIJ").is_some();
    app.configure_image_terminal(
        detect(&term, &program, multiplexed),
        pixels(),
    );
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
    if multiplexed {
        return ImageProtocol::Blocks;
    }
    if matches!(term, "foot" | "foot-extra") || program == "foot" {
        return ImageProtocol::Sixel;
    }
    if matches!(program, "iTerm.app" | "WezTerm") {
        return ImageProtocol::Iterm2;
    }
    ImageProtocol::Blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_images_need_explicit_supported_terminal_evidence() {
        for (term, program, expected) in [
            ("xterm-256color", "", ImageProtocol::Blocks),
            ("xterm-kitty", "", ImageProtocol::Blocks),
            ("xterm-ghostty", "ghostty", ImageProtocol::Blocks),
            ("xterm-256color", "WezTerm", ImageProtocol::Iterm2),
            ("xterm-256color", "iTerm.app", ImageProtocol::Iterm2),
            ("foot", "", ImageProtocol::Sixel),
            ("foot-extra", "", ImageProtocol::Sixel),
            ("xterm-256color", "foot", ImageProtocol::Sixel),
        ] {
            assert_eq!(detect(term, program, false), expected);
            assert_eq!(detect(term, program, true), ImageProtocol::Blocks);
        }
    }
}
