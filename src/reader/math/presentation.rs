//! MathML structure selects spatial graphics; source spelling never forces prose out.
use ratatui::style::Modifier;
use roxmltree::Node;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::reader) struct Text {
    pub(in crate::reader) value: String,
    pub(in crate::reader) spatial: bool,
    pub(in crate::reader) modifier: Modifier,
}

impl Text {
    pub(super) fn new(root: Node<'_, '_>, value: String) -> Self {
        let spatial =
            root.descendants()
                .filter(Node::is_element)
                .any(|node| match node.tag_name().name() {
                    "mfrac" | "msqrt" | "mroot" | "mover" | "munder"
                    | "munderover" | "mtable" | "menclose" => true,
                    "msub" | "msup" | "msubsup" | "mmultiscripts" => {
                        value.contains(['_', '^'])
                    }
                    _ => false,
                });
        Self {
            value,
            spatial,
            modifier: uniform_modifier(root),
        }
    }
}

fn uniform_modifier(mut node: Node<'_, '_>) -> Modifier {
    let mut modifier = Modifier::empty();
    loop {
        modifier |= match node.tag_name().name() {
            "b" => Modifier::BOLD,
            "i" | "em" => Modifier::ITALIC,
            "s" => Modifier::CROSSED_OUT,
            "u" => Modifier::UNDERLINED,
            _ => Modifier::empty(),
        };
        let mut elements = node.children().filter(Node::is_element);
        let Some(child) = elements.next() else {
            return modifier;
        };
        if elements.next().is_some()
            || node.children().any(|child| {
                child.is_text()
                    && child.text().is_some_and(|text| !text.trim().is_empty())
            })
        {
            return modifier;
        }
        node = child;
    }
}
