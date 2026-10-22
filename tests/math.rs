//! Formula readability and source fidelity across terminal layouts.
use anyhow::{Context, Result};
use lume::reader;

#[test]
fn latex_composition_is_readable_and_keeps_original_annotation_source()
-> Result<()> {
    let formula = r"$R{\mathcal{I}}=\text{Combine}\circ\text{Disintegrate}$";
    let body = format!("Before {formula} after.");
    for width in [12, 80] {
        let rendered = reader::render(&body, width);
        assert!(
            rendered.text.contains("Rℐ = Combine ∘ Disintegrate"),
            "{}",
            rendered.text
        );
        let start = rendered.text.find("Rℐ").context("Formula missing")?;
        let end = rendered
            .text
            .find("Disintegrate")
            .context("Operator lost")?
            + "Disintegrate".len();
        let source = reader::to_source(&rendered.mappings, start..end)
            .context("Source lost")?;
        assert_eq!(&body[source], formula);
    }
    Ok(())
}

#[test]
fn fractions_scripts_roots_and_display_formulas_survive_wrapping() {
    let body = r"Inline $x^2+y_1=\frac{a}{b}$.

$$
\sqrt{x}+\alpha\leq\beta
$$

After.";
    let rendered = reader::render(body, 80);
    assert!(
        rendered.text.contains("x² + y₁ = (a)/(b)"),
        "{}",
        rendered.text
    );
    assert!(rendered.text.contains("√(x) + α ≤ β"), "{}", rendered.text);
    assert!(rendered.text.contains("After."));
}

#[test]
fn unsupported_and_excessive_math_remains_visible_and_code_is_literal() {
    for formula in [
        r"$\unknown{x}$".to_owned(),
        format!("${}x{}$", "{".repeat(80), "}".repeat(80)),
        format!("${}$", "x".repeat(4097)),
    ] {
        let rendered = reader::render(&formula, 80);
        assert!(rendered.text.contains(&formula));
    }
    let code = reader::render("`$x^2$`\n\n```tex\n$x^2$\n```", 80);
    assert_eq!(code.text.matches("$x^2$").count(), 2);
    assert_eq!(
        reader::render("Costs $5 today.", 80).text.trim(),
        "Costs $5 today."
    );
}

#[test]
fn excessive_formula_counts_fall_back_without_losing_content() {
    let body = "$x^2$ ".repeat(300);
    let rendered = reader::render(&body, 80);
    assert_eq!(rendered.text.matches("x²").count(), 256);
    assert_eq!(rendered.text.matches("$x^2$").count(), 44);
}

#[test]
fn paired_currency_and_unsupported_layout_commands_keep_their_source() {
    for text in ["Costs $5 and $10 today.", r"$\phantom{x}$"] {
        assert_eq!(reader::render(text, 80).text.trim(), text);
    }
}

#[test]
fn sums_products_integrals_and_limits_have_readable_bounds() {
    for (latex, expected) in [
        (r"\sum_{i=1}^{n} x_i^2", "∑ᵢ₌₁ⁿ xᵢ²"),
        (r"\prod_{k=1}^{n} k", "∏ₖ₌₁ⁿ k"),
        (r"\int_0^\infty e^{-x}\,dx", "∫₀^(∞) e⁻ˣ dx"),
        (r"\lim_{x\to0}\frac{\sin x}{x}", "lim_(x→0)"),
    ] {
        let rendered = reader::render(&format!("$$\n{latex}\n$$"), 80);
        assert!(
            rendered.text.contains(expected),
            "{latex}: {}",
            rendered.text
        );
        assert!(
            !rendered.text.contains('\\'),
            "Unconverted formula: {}",
            rendered.text
        );
    }
}

#[test]
fn extended_scripts_accents_matrices_and_binomials_keep_their_meaning() {
    for (latex, expected) in [
        (r"x^{abc}+y_{max}+A_{ij}^{-1}", "xᵃᵇᶜ + yₘₐₓ + Aᵢⱼ⁻¹"),
        (
            r"\hat{x}+\bar{v}+\vec{u}+\dot{x}+\ddot{y}",
            "x\u{0302} + v\u{0305} + u\u{20d7} + x\u{0307} + y\u{0308}",
        ),
        (r"\binom{n}{k}", "(n choose k)"),
        (r"\begin{pmatrix}a&b\\c&d\end{pmatrix}", "(a, b; c, d)"),
        (
            r"\begin{cases}x^2&x>0\\0&\text{otherwise}\end{cases}",
            "{x², x > 0; 0, otherwise",
        ),
        (r"\mathbb{R}\subseteq\mathbb{C}", "ℝ ⊆ ℂ"),
        (
            r"\begin{aligned}a&=b+c\\x&=y\end{aligned}",
            "[a = b + c; x = y]",
        ),
        (r"\sin(x)+\log_2 x", "sin (x) + log₂ x"),
    ] {
        let rendered = reader::render(&format!("${latex}$"), 80);
        assert!(
            rendered.text.contains(expected),
            "{latex}: {}",
            rendered.text
        );
    }
}

#[test]
fn disabled_formulas_keep_the_exact_raw_source_even_in_tables_and_headings() {
    let body = "# $x^2$\n\nInline $x^2$.\n\n$$\n\\sum_{i=1}^n x_i\n$$\n\n| Formula |\n| --- |\n| $y_1$ |";
    let options = reader::RenderOptions {
        latex_formulas: false,
    };
    let rendered = reader::render_with_options(body, 80, options);
    assert!(rendered.text.contains("$x^2$"));
    assert!(rendered.text.contains("\\sum_{i=1}^n x_i"));
    assert!(rendered.text.contains("$y_1$"));
    assert!(!rendered.text.contains("x²"));
    assert_eq!(
        reader::index::DocumentIndex::parse_with_options(body, options)
            .headings[0]
            .title,
        "$x^2$"
    );
}
