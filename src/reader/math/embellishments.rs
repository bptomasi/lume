//! Selectable notation for enclosures and tensor indices with exact source maps.
use super::{projection::project, scripts::script};
use roxmltree::Node;

pub(super) fn enclosure(
    node: Node<'_, '_>,
    parts: &[String],
) -> Option<String> {
    let name = match node.attribute("notation")? {
        "box" => "boxed",
        "updiagonalstrike" => "cancel",
        "downdiagonalstrike" => "bcancel",
        "updiagonalstrike downdiagonalstrike"
        | "downdiagonalstrike updiagonalstrike" => "xcancel",
        _ => return None,
    };
    Some(format!("{name}({})", parts.concat()))
}

pub(super) fn tensor(node: Node<'_, '_>, depth: usize) -> Option<String> {
    let mut children = node.children().filter(Node::is_element);
    let base = super::projection::scripted_base(
        node,
        &project(children.next()?, depth + 1)?,
    );
    let mut before = String::new();
    let mut after = String::new();
    let mut prescripts = false;
    while let Some(lower) = children.next() {
        if lower.has_tag_name("mprescripts") {
            if prescripts {
                return None;
            }
            prescripts = true;
            continue;
        }
        let upper = children.next()?;
        let target = if prescripts { &mut before } else { &mut after };
        for (node, superscript) in [(lower, false), (upper, true)] {
            if !node.has_tag_name("none") {
                target
                    .push_str(&script(&project(node, depth + 1)?, superscript));
            }
        }
    }
    Some(format!("{before}{base}{after}"))
}
