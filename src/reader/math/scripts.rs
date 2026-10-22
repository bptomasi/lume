//! Unicode scripts stay atomic: missing glyphs retain an explicit grouped script.

pub(super) fn script(text: &str, upper: bool) -> String {
    let result: Option<String> =
        text.chars().map(|c| glyph(c, upper)).collect();
    result
        .unwrap_or_else(|| format!("{}({text})", if upper { '^' } else { '_' }))
}

fn glyph(character: char, upper: bool) -> Option<char> {
    let digits = "0123456789+-−=()";
    let numbers = if upper {
        "⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻⁻⁼⁽⁾"
    } else {
        "₀₁₂₃₄₅₆₇₈₉₊₋₋₌₍₎"
    };
    let (latin, letters, greek, symbols) = if upper {
        (
            "abcdefghijklmnoprstuvwxyzABDEGHIJKLMNOPRTUVW",
            "ᵃᵇᶜᵈᵉᶠᵍʰⁱʲᵏˡᵐⁿᵒᵖʳˢᵗᵘᵛʷˣʸᶻᴬᴮᴰᴱᴳᴴᴵᴶᴷᴸᴹᴺᴼᴾᴿᵀᵁⱽᵂ",
            "αβγδεθφχ",
            "ᵅᵝᵞᵟᵋᶿᵠᵡ",
        )
    } else {
        ("aehijklmnoprstuvx", "ₐₑₕᵢⱼₖₗₘₙₒₚᵣₛₜᵤᵥₓ", "βγρφχ", "ᵦᵧᵨᵩᵪ")
    };
    digits
        .chars()
        .zip(numbers.chars())
        .chain(latin.chars().zip(letters.chars()))
        .chain(greek.chars().zip(symbols.chars()))
        .find_map(|(normal, script)| (normal == character).then_some(script))
}
