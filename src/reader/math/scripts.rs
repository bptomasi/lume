//! Unicode scripts stay atomic: missing glyphs retain an explicit grouped script.

pub(super) fn script(text: &str, upper: bool) -> String {
    let expanded = super::fractions::expand(text);
    let result: Option<String> =
        expanded.chars().map(|c| glyph(c, upper)).collect();
    result.unwrap_or_else(|| {
        let marker = if upper { '^' } else { '_' };
        if text.chars().count() == 1 {
            format!("{marker}{text}")
        } else {
            format!("{marker}({text})")
        }
    })
}

fn glyph(character: char, upper: bool) -> Option<char> {
    if matches!(character, '/' | '⁄') {
        return Some('⁄');
    }
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
