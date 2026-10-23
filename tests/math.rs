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
    assert!(rendered.text.contains("x² + y₁ = a/b"), "{}", rendered.text);
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
        (r"\int_0^\infty e^{-x}\,dx", "∫₀^∞ e⁻ˣ dx"),
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
        graphical_formulas: false,
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

#[test]
fn fractional_powers_and_grouped_bases_preserve_arithmetic_meaning() {
    for (source, expected) in [
        (r"$O(\log^{3/2} n)$", "O(log³⁄² n)"),
        (r"${a+b}^2$", "(a + b)²"),
        (r"$\frac{1}{2}$", "½"),
        (r"$\frac{a+b}{c+d}$", "(a+b)/(c+d)"),
        (r"$x^{\frac{3}{2}}$", "x³⁄²"),
        (r"$x^{\frac{1}{2}}$", "x¹⁄²"),
        (r"$\frac{\frac{a}{b}}{c}$", "(a/b)/c"),
    ] {
        let rendered = reader::render(source, 80);
        assert!(
            rendered.text.contains(expected),
            "{source}: {}",
            rendered.text
        );
    }
}

#[test]
fn nested_grouped_bases_and_vulgar_fraction_scripts_remain_unambiguous() {
    for (source, expected) in [
        (r"${(a)+(b)}^2$", "((a) + (b))²"),
        (r"$x^{\frac{1}{5}}$", "x¹⁄⁵"),
        (r"$x^{\frac{5}{6}}$", "x⁵⁄⁶"),
    ] {
        let rendered = reader::render(source, 80);
        assert!(
            rendered.text.contains(expected),
            "{source}: {}",
            rendered.text
        );
    }
}

#[test]
fn powers_of_powers_and_fraction_bases_keep_their_grouping() {
    for (source, expected) in [
        (r"${x^2}^3$", "(x²)³"),
        (r"${x_i^2}^3$", "(xᵢ²)³"),
        (r"${\frac{1}{2}}^2$", "(½)²"),
    ] {
        let rendered = reader::render(source, 80);
        assert!(
            rendered.text.contains(expected),
            "{source}: {}",
            rendered.text
        );
    }
}

#[test]
fn boxed_cancelled_braced_and_prescript_expressions_keep_selectable_source()
-> Result<()> {
    for (latex, expected) in [
        (r"\boxed{x^2+y^2}", "boxed(x² + y²)"),
        (r"\cancel{x+y}", "cancel(x + y)"),
        (r"\bcancel{x}", "bcancel(x)"),
        (r"\xcancel{x+y}", "xcancel(x + y)"),
        (r"\overbrace{a+b}^{n}", "overbrace(a + b)ⁿ"),
        (r"\underbrace{a+b}_{n}", "underbrace(a + b)ₙ"),
        (r"\prescript{14}{6}{C}", "₆¹⁴C"),
        (r"\prescript{a}{b}{\frac{x}{y}}_i^n", "_bᵃ(x/y)ᵢⁿ"),
        (r"\utilde{xy}", "utilde(xy)"),
        (r"\widecheck{xy}", "check(xy)"),
        (r"\prescript{a}{b}{X}_c^d", "_bᵃX_cᵈ"),
        (r"\underleftarrow{AB}", "underleftarrow(AB)"),
        (r"\widehat{xyz}", "hat(xyz)"),
        (r"\widetilde{xyz}", "tilde(xyz)"),
    ] {
        let source = format!("Before ${latex}$ after.");
        let rendered = reader::render(&source, 120);
        assert!(
            rendered.text.contains(expected),
            "{latex}: {}",
            rendered.text
        );
        let first = rendered
            .text
            .find(expected)
            .context("Formula text missing")?;
        let range = reader::to_source(
            &rendered.mappings,
            first..first + expected.len(),
        )
        .context("Formula source mapping missing")?;
        assert_eq!(&source[range], format!("${latex}$"));
    }
    Ok(())
}

#[test]
fn repeated_unsupported_projection_keeps_every_original_expression() {
    let formula = r"$\phantom{\frac{a+b+c+d}{\sqrt{x^2+y^2}}}$";
    let body = format!("{formula}\n\n").repeat(300);
    let rendered = reader::render(&body, 80);
    assert_eq!(rendered.text.matches(formula).count(), 300);
}
