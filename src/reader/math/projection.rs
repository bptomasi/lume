//! MathML semantics become bounded, portable and selectable terminal text.
use super::scripts::script;
use roxmltree::Node;
use unicode_segmentation::UnicodeSegmentation;

pub(super) fn project(node: Node<'_, '_>, depth: usize) -> Option<String> {
    if depth > 64 {
        return None;
    }
    if node.is_text() {
        return Some(node.text().unwrap_or_default().into());
    }
    let parts: Vec<String> = node
        .children()
        .filter(|child| child.is_element() || child.is_text())
        .map(|child| project(child, depth + 1))
        .collect::<Option<_>>()?;
    let text = match (node.tag_name().name(), parts.as_slice()) {
        ("mfrac", [a, b]) if node.attribute("linethickness") == Some("0") => {
            format!("{a} choose {b}")
        }
        ("mfrac", [a, b]) => fraction(a, b),
        ("msqrt", _) => format!("√({})", parts.concat()),
        ("mroot", [a, b]) => format!("{}√({a})", script(b, true)),
        ("msup", [a, b]) => limits(&scripted_base(node, a), None, Some(b)),
        ("msub", [a, b]) => limits(&scripted_base(node, a), Some(b), None),
        ("msubsup", [a, b, c]) => {
            limits(&scripted_base(node, a), Some(b), Some(c))
        }
        ("mover" | "munder" | "munderover", _) => decorated(node, &parts)?,
        ("mtable", _) => matrix(node, &parts),
        ("mtr", _) => matrix_row(node, &parts),
        ("mspace", _) => node
            .attribute("width")
            .map_or_else(String::new, |_| " ".into()),
        ("mi", _) => parts.concat().replace(['\u{fe00}', '\u{fe01}'], ""),
        ("annotation", _) => String::new(),
        ("mo", _) => operator(node, &parts.concat()),
        (
            "math" | "mrow" | "mn" | "mtext" | "mstyle" | "mtd" | "semantics",
            _,
        ) => parts.concat(),
        _ => return None,
    };
    Some(text)
}

fn fraction(numerator: &str, denominator: &str) -> String {
    super::fractions::compact(numerator, denominator).map_or_else(
        || format!("{}/{}", script_base(numerator), script_base(denominator)),
        |glyph| glyph.to_string(),
    )
}

fn scripted_base(node: Node<'_, '_>, text: &str) -> String {
    let mut base = node.first_element_child();
    while let Some(child) = base {
        if child.has_tag_name("mrow")
            && child.children().filter(Node::is_element).count() == 1
        {
            base = child.first_element_child();
        } else {
            break;
        }
    }
    if base.is_some_and(|base| {
        matches!(
            base.tag_name().name(),
            "msup" | "msubsup" | "mfrac" | "msqrt" | "mroot"
        )
    }) {
        format!("({text})")
    } else {
        script_base(text)
    }
}

fn script_base(text: &str) -> String {
    if text.contains(['+', '−', '/', '=', ' ']) && !outer_group(text) {
        format!("({text})")
    } else {
        text.to_owned()
    }
}

fn outer_group(text: &str) -> bool {
    if !text.starts_with('(') || !text.ends_with(')') {
        return false;
    }
    let mut depth = 0_usize;
    for (offset, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth == 0 && offset + character.len_utf8() < text.len() {
            return false;
        }
    }
    depth == 0
}

fn limits(base: &str, lower: Option<&str>, upper: Option<&str>) -> String {
    let mut text = base.to_owned();
    if let Some(lower) = lower {
        text.push_str(&script(lower, false));
    }
    if let Some(upper) = upper {
        text.push_str(&script(upper, true));
    }
    if matches!(base, "∑" | "∏" | "∐" | "∫" | "∬" | "∭" | "∮" | "⋃" | "⋂")
    {
        text.push(' ');
    }
    text
}

fn decorated(node: Node<'_, '_>, parts: &[String]) -> Option<String> {
    match (node.tag_name().name(), parts) {
        ("mover", [base, mark]) if node.attribute("accent") == Some("true") => {
            accent(base, mark, false)
        }
        ("munder", [base, mark])
            if node.attribute("accentunder") == Some("true") =>
        {
            accent(base, mark, true)
        }
        ("mover", [base, upper]) => Some(limits(base, None, Some(upper))),
        ("munder", [base, lower]) => Some(limits(base, Some(lower), None)),
        ("munderover", [base, lower, upper]) => {
            Some(limits(base, Some(lower), Some(upper)))
        }
        _ => None,
    }
}

fn accent(base: &str, mark: &str, under: bool) -> Option<String> {
    let (name, combining) = match mark {
        "ˆ" | "^" => ("hat", '\u{0302}'),
        "¯" | "‾" | "_" if under => ("underline", '\u{0332}'),
        "¯" | "‾" => ("overline", '\u{0305}'),
        "⃗" | "→" => ("vec", '\u{20d7}'),
        "˙" | "." => ("dot", '\u{0307}'),
        "¨" => ("ddot", '\u{0308}'),
        "˜" | "~" => ("tilde", '\u{0303}'),
        "ˇ" => ("check", '\u{030c}'),
        "˘" => ("breve", '\u{0306}'),
        _ => return None,
    };
    Some(if base.graphemes(true).count() == 1 {
        format!("{base}{combining}")
    } else {
        format!("{name}({base})")
    })
}

fn matrix_row(node: Node<'_, '_>, cells: &[String]) -> String {
    if node
        .parent_element()
        .is_some_and(|n| n.attribute("displaystyle") == Some("true"))
    {
        cells
            .chunks(2)
            .map(|pair| pair.concat())
            .collect::<Vec<_>>()
            .join("; ")
    } else {
        cells.join(", ")
    }
}

fn matrix(node: Node<'_, '_>, rows: &[String]) -> String {
    let fenced = node
        .prev_sibling_element()
        .is_some_and(|n| n.tag_name().name() == "mo")
        && node
            .parent_element()
            .is_some_and(|n| n.tag_name().name() == "mrow");
    if fenced {
        rows.join("; ")
    } else {
        format!("[{}]", rows.join("; "))
    }
}

fn operator(node: Node<'_, '_>, text: &str) -> String {
    match text {
        "\u{2061}" => return " ".into(),
        "\u{2062}" => return "·".into(),
        "\u{2063}" => return String::new(),
        _ => {}
    }
    let tight = node.attribute("lspace") == Some("0")
        && node.attribute("rspace") == Some("0");
    if !tight
        && matches!(
            text,
            "=" | "+"
                | "−"
                | "×"
                | "∘"
                | "≤"
                | "≥"
                | "∈"
                | "→"
                | "<"
                | ">"
                | "≠"
                | "≈"
                | "≡"
                | "⊂"
                | "⊆"
                | "⊃"
                | "⊇"
                | "∧"
                | "∨"
                | "⇒"
                | "⇔"
        )
    {
        format!(" {text} ")
    } else {
        text.into()
    }
}
