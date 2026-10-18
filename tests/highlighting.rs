//! Language and publisher contracts are checked through rendered reader output.

#[path = "fixtures/code_samples.rs"]
mod samples;

use anyhow::{Result, anyhow, ensure};
use lume::{content, reader};
use ratatui::{style::Color, text::Line};

fn code_text(rendered: &reader::Rendered) -> Vec<String> {
    rendered
        .lines
        .iter()
        .zip(&rendered.code)
        .filter(|(_, code)| **code)
        .map(|(line, _)| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect()
        })
        .collect()
}

fn token_color(lines: &[Line<'_>], token: &str) -> Result<Color> {
    for line in lines {
        let text = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        let Some(start) = text.find(token) else {
            continue;
        };
        let mut offset = 0;
        for span in &line.spans {
            let end = offset + span.content.len();
            if offset <= start && start < end {
                return span
                    .style
                    .fg
                    .ok_or_else(|| anyhow!("Uncolored token {token}"));
            }
            offset = end;
        }
    }
    Err(anyhow!("Token missing: {token}"))
}

#[test]
fn every_supported_language_preserves_source_and_colors_its_tokens()
-> Result<()> {
    for sample in samples::SAMPLES {
        let markdown = format!("```{}\n{}```\n", sample.language, sample.code);
        let rendered = reader::render(&markdown, 20);
        ensure!(
            code_text(&rendered) == sample.code.lines().collect::<Vec<_>>(),
            "{} changed source or wrapped code",
            sample.language
        );
        ensure!(
            token_color(&rendered.lines, sample.token)? == sample.color,
            "{} token {} has incorrect color",
            sample.language,
            sample.token
        );
        ensure!(
            token_color(&rendered.lines, "hello")? == Color::Rgb(166, 191, 145),
            "{} string is not highlighted",
            sample.language
        );
    }
    Ok(())
}

#[test]
fn familiar_aliases_resolve_to_the_same_grammar() {
    for (canonical, aliases) in [
        ("rust", &["rs", "Rust", "rust,ignore", "{.rust}"][..]),
        ("terraform", &["hcl", "tf", "tfvars"][..]),
        ("javascript", &["js", "nodejs", "language-javascript"][..]),
        ("typescript", &["ts", "mts", "cts", "lang-typescript"][..]),
        ("go", &["golang"][..]),
        ("cpp", &["c++", "cxx"][..]),
        ("bash", &["sh", "shell", "shell-session"][..]),
        ("yaml", &["yml"][..]),
    ] {
        let source = samples::SAMPLES
            .iter()
            .find(|s| s.language == canonical)
            .map_or("", |sample| sample.code);
        let expected =
            reader::render(&format!("```{canonical}\n{source}```"), 80);
        for alias in aliases {
            let actual =
                reader::render(&format!("```{alias}\n{source}```"), 80);
            assert_eq!(actual.lines, expected.lines, "Alias {alias}");
        }
    }
}

#[test]
fn html_code_and_pre_metadata_survive_normalization() -> Result<()> {
    for fragment in [
        "<pre><code class='language-typescript'>const x = &quot;hello&quot;;</code></pre>",
        "<pre class='lang-typescript'><code>const x = &quot;hello&quot;;</code></pre>",
        "<pre data-lang='typescript'>const x = &quot;hello&quot;;</pre>",
        "<pre><code data-language='typescript'>const x = &quot;hello&quot;;</code></pre>",
        "<pre><code class='highlight-source-typescript'>const x = &quot;hello&quot;;</code></pre>",
    ] {
        let markdown = content::article_text(fragment, true);
        assert!(markdown.starts_with("~~~~typescript\n"), "{markdown}");
        let rendered = reader::render(&markdown, 80);
        assert_eq!(code_text(&rendered), ["const x = \"hello\";"]);
        assert_eq!(
            token_color(&rendered.lines, "const")?,
            Color::Rgb(216, 164, 116)
        );
    }
    Ok(())
}

#[test]
fn multiline_comments_keep_their_lexical_state() -> Result<()> {
    let source =
        "```c\n/* explanation\n   continued comment */\nint value = 42;\n```";
    let rendered = reader::render(source, 80);
    assert_eq!(
        token_color(&rendered.lines, "continued")?,
        Color::Rgb(124, 129, 132)
    );
    assert_eq!(
        token_color(&rendered.lines, "42")?,
        Color::Rgb(194, 162, 195)
    );
    Ok(())
}

#[test]
fn literal_fences_and_code_metadata_cannot_escape_the_code_block() {
    let fragment = "<pre class='language-rust'><code data-lang='c'>const char *x = &quot;hello&quot;;\n~~~~\n  return 0;  \n</code></pre><p>After</p>";
    let markdown = content::article_text(fragment, true);
    assert!(markdown.starts_with("~~~~~c\n"));
    let rendered = reader::render(&markdown, 80);
    assert_eq!(
        code_text(&rendered),
        ["const char *x = \"hello\";", "~~~~", "  return 0;  "]
    );
    assert!(
        rendered
            .lines
            .iter()
            .any(|line| line.to_string() == "After")
    );
}

#[test]
fn unknown_missing_and_oversized_languages_keep_readable_code() {
    let source = "  unrecognized <T> & value\n";
    for language in ["", "not-a-language"] {
        let rendered =
            reader::render(&format!("```{language}\n{source}```"), 80);
        assert_eq!(code_text(&rendered), ["  unrecognized <T> & value"]);
    }
    let long_line = "x".repeat(4097);
    let rendered =
        reader::render(&format!("```typescript\n{long_line}\n```"), 80);
    assert_eq!(code_text(&rendered), [long_line]);
    assert!(
        rendered.lines[0]
            .spans
            .iter()
            .all(|span| span.style.fg.is_none())
    );
}
