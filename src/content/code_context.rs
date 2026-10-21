//! Publisher wrappers can label code without annotating the pre/code element.

use html5gum::StartTag;

#[derive(Default)]
pub(super) struct CodeContext {
    frames: Vec<(String, Option<String>)>,
    overflow: usize,
}

impl CodeContext {
    pub(super) fn start(&mut self, tag: &StartTag<()>) {
        let name = String::from_utf8_lossy(&tag.name);
        if !matches!(
            name.as_ref(),
            "div" | "figure" | "section" | "article" | "span"
        ) {
            return;
        }
        if self.frames.len() == 128 {
            self.overflow += 1;
            return;
        }
        self.frames
            .push((name.into_owned(), super::code::language(tag)));
    }

    pub(super) fn end(&mut self, name: &str) {
        if self.overflow > 0 {
            if matches!(name, "div" | "figure" | "section" | "article" | "span")
            {
                self.overflow -= 1;
            }
            return;
        }
        if let Some(index) =
            self.frames.iter().rposition(|(tag, _)| tag == name)
        {
            self.frames.truncate(index);
        }
    }

    pub(super) fn language(&self) -> Option<&str> {
        if self.overflow > 0 {
            return None;
        }
        self.frames
            .iter()
            .rev()
            .find_map(|(_, language)| language.as_deref())
    }
}
