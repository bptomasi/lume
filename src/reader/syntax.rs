//! Embedded, reviewed text grammars and bounded code highlighting.
//!
//! The build compiles grammars; loading is shared for the process lifetime.
//! Unknown languages, large blocks and failures retain readable plain code.

use std::sync::OnceLock;

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Color as SyntaxColor, Style as SyntaxStyle, Theme},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

static SYNTAXES: OnceLock<Result<SyntaxSet, String>> = OnceLock::new();
static THEME: OnceLock<Theme> = OnceLock::new();

fn syntaxes() -> &'static Result<SyntaxSet, String> {
    SYNTAXES.get_or_init(|| {
        let bytes =
            include_bytes!(concat!(env!("OUT_DIR"), "/reader-syntaxes.pack"));
        syntect::dumps::from_reader(bytes.as_slice())
            .map_err(|error| error.to_string())
    })
}

pub(super) fn highlight(text: &str, language: &str) -> Vec<Line<'static>> {
    let Ok(syntaxes) = syntaxes() else {
        return plain(text);
    };
    if text.len() > 65_536 || text.lines().any(|line| line.len() > 4096) {
        return plain(text);
    }
    let language = super::language::resolve(text, language);
    let syntax = (if language == "sql" {
        syntaxes.find_syntax_by_name("Lume SQL")
    } else {
        syntaxes.find_syntax_by_token(&language)
    })
    .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
    let mut highlighter = HighlightLines::new(syntax, THEME.get_or_init(theme));
    LinesWithEndings::from(text)
        .map(|line| {
            let Ok(ranges) = highlighter.highlight_line(line, syntaxes) else {
                return Line::raw(line.trim_end_matches('\n').to_owned());
            };
            Line::from(
                ranges
                    .into_iter()
                    .map(|(style, text)| {
                        Span::styled(
                            text.trim_end_matches('\n').to_owned(),
                            terminal_style(style),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

fn plain(text: &str) -> Vec<Line<'static>> {
    text.lines()
        .map(|line| Line::raw(line.to_owned()))
        .collect()
}

fn terminal_style(style: SyntaxStyle) -> Style {
    Style::new().fg(Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    ))
}

fn theme() -> Theme {
    let mut theme = Theme::default();
    theme.settings.foreground = Some(SyntaxColor {
        r: 207,
        g: 207,
        b: 199,
        a: 255,
    });
    for (selector, (r, g, b)) in [
        ("keyword, storage", (216, 164, 116)),
        ("string", (166, 191, 145)),
        ("comment", (124, 129, 132)),
        ("entity.name.function, support.function", (146, 183, 202)),
        (
            "entity.name.type, entity.name.class, support.type",
            (225, 193, 140),
        ),
        ("constant.numeric, constant.language", (194, 162, 195)),
    ] {
        if let Ok(scope) = selector.parse() {
            theme.scopes.push(syntect::highlighting::ThemeItem {
                scope,
                style: syntect::highlighting::StyleModifier {
                    foreground: Some(SyntaxColor { r, g, b, a: 255 }),
                    ..Default::default()
                },
            });
        }
    }
    theme
}

#[cfg(test)]
mod tests {
    #[test]
    fn embedded_grammars_compile_and_color_rust() {
        assert!(super::syntaxes().is_ok(), "{:?}", super::syntaxes());
        let lines = super::highlight("fn main() { let n = 42; }\n", "rust");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].spans.len() > 3);
    }
}
