//! Correctness-checked formula preparation and cache policy transitions.
use anyhow::{Result, ensure};
use lume::reader::{self, LiveRenderer, RenderOptions};

const EXPRESSIONS: &[(&str, &str)] = &[
    (r"\sum_{i=1}^{n} x_i^2", "∑ᵢ₌₁ⁿ xᵢ²"),
    (r"\int_0^\infty e^{-x}\,dx", "∫₀^(∞) e⁻ˣ dx"),
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
    cached_transitions(&body, iterations)
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
