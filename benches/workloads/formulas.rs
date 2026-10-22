//! Correctness-checked formula preparation and cache policy transitions.
use anyhow::{Result, ensure};
use lume::reader::{self, LiveRenderer, RenderOptions};

const EXPRESSIONS: &[(&str, &str)] = &[
    (r"\sum_{i=1}^{n} x_i^2", "∑ᵢ₌₁ⁿ xᵢ²"),
    (r"\int_0^\infty e^{-x}\,dx", "∫₀^∞ e⁻ˣ dx"),
    (r"\hat{x}+\vec{u}", "x\u{0302} + u\u{20d7}"),
    (r"\begin{pmatrix}a&b\\c&d\end{pmatrix}", "(a, b; c, d)"),
    (r"\sin(x)+\log_2 x", "sin (x) + log₂ x"),
];

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let copies = if stress { 50 } else { 8 };
    let body = EXPRESSIONS
        .iter()
        .map(|(tex, _)| format!("${tex}$\n\n"))
        .collect::<String>()
        .repeat(copies);
    for enabled in [true, false] {
        let options = RenderOptions {
            latex_formulas: enabled,
            graphical_formulas: false,
        };
        let label = if enabled {
            "advanced formula parse + Unicode layout"
        } else {
            "raw formula layout without LaTeX conversion"
        };
        super::measure(label, iterations, || {
            let rendered = reader::render_with_options(&body, 80, options);
            for (raw, converted) in EXPRESSIONS {
                let expected = if enabled {
                    converted.to_string()
                } else {
                    format!("${raw}$")
                };
                ensure!(
                    rendered.text.matches(&expected).count() == copies,
                    "Formula text or budget changed"
                );
            }
            Ok(())
        })?;
    }
    graphical_layout(&body.replace('$', "$$"), iterations)?;
    compact_abstract(iterations)?;
    cached_transitions(&body, iterations)
}

fn compact_abstract(iterations: usize) -> Result<()> {
    let body = include_str!("../../tests/fixtures/arxiv-tool-access.md");
    super::measure("compact mixed arXiv formula layout", iterations, || {
        let rendered = reader::render_with_options(
            body,
            80,
            RenderOptions {
                latex_formulas: true,
                graphical_formulas: true,
            },
        );
        ensure!(rendered.images.len() == 2, "Inline variables became images");
        ensure!(rendered.lines.len() < 24, "Abstract became fragmented");
        ensure!(
            rendered.images.iter().all(|slot| slot.rows.len() <= 2),
            "Formula exceeded its compact font size"
        );
        Ok(())
    })
}

fn cached_transitions(body: &str, iterations: usize) -> Result<()> {
    let mut renderer = LiveRenderer::start()?;
    super::measure(
        "cached formula policy round trip + repeat renders",
        iterations,
        || {
            for enabled in [false, true] {
                renderer.set_latex_formulas(enabled);
                let rendered = renderer.render(body, 80);
                ensure!(
                    rendered.text.contains("∑") == enabled,
                    "Obsolete policy escaped cache"
                );
                let cached = renderer.render(body, 80);
                ensure!(
                    cached.text == rendered.text,
                    "Cache changed formula text"
                );
            }
            Ok(())
        },
    )
}

fn graphical_layout(body: &str, iterations: usize) -> Result<()> {
    super::measure(
        "vector formula layout + shared source outlines",
        iterations,
        || {
            let rendered = reader::render_with_options(
                body,
                80,
                RenderOptions {
                    latex_formulas: true,
                    graphical_formulas: true,
                },
            );
            ensure!(
                rendered.images.len() == 32,
                "Formula image budget changed"
            );
            for slot in &rendered.images {
                let formula = slot
                    .formula
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Remote formula image"))?;
                ensure!(!formula.svg.is_empty(), "Formula outlines missing");
                ensure!(
                    body[slot.source.clone()].contains(&*formula.latex),
                    "Source drifted"
                );
            }
            Ok(())
        },
    )
}
