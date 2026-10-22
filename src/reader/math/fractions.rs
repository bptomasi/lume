//! Unicode vulgar fractions and script expansion share one exact ratio table.
const RATIOS: &[(char, &str, &str)] = &[
    ('½', "1", "2"),
    ('⅓', "1", "3"),
    ('⅔', "2", "3"),
    ('¼', "1", "4"),
    ('¾', "3", "4"),
    ('⅕', "1", "5"),
    ('⅖', "2", "5"),
    ('⅗', "3", "5"),
    ('⅘', "4", "5"),
    ('⅙', "1", "6"),
    ('⅚', "5", "6"),
    ('⅐', "1", "7"),
    ('⅛', "1", "8"),
    ('⅜', "3", "8"),
    ('⅝', "5", "8"),
    ('⅞', "7", "8"),
    ('⅑', "1", "9"),
    ('⅒', "1", "10"),
];

pub(super) fn compact(numerator: &str, denominator: &str) -> Option<char> {
    RATIOS.iter().find_map(|(glyph, top, bottom)| {
        (*top == numerator && *bottom == denominator).then_some(*glyph)
    })
}

pub(super) fn expand(text: &str) -> String {
    let mut expanded = String::with_capacity(text.len());
    for character in text.chars() {
        if let Some((_, top, bottom)) =
            RATIOS.iter().find(|(glyph, _, _)| *glyph == character)
        {
            expanded.push_str(top);
            expanded.push('⁄');
            expanded.push_str(bottom);
        } else {
            expanded.push(character);
        }
    }
    expanded
}
