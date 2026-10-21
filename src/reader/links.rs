//! Link identity and bounded labels are independent of wrapped visual rows.
//!
//! Parser events supply destinations and label text in encounter order; the
//! existing URL contract admits only supported HTTP(S) references.
//! A label survives hard breaks and image slots; repeated destinations keep their
//! original position and most recent nonempty label. No paragraph prefix is copied.

use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Links {
    entries: Vec<(String, String)>,
    indices: HashMap<String, usize>,
    active: Option<Label>,
}

struct Label {
    index: usize,
    text: String,
    characters: usize,
}

impl Links {
    pub(super) fn begin(&mut self, address: &str) {
        self.active = None;
        let Ok(url) = crate::source::link_url(address) else {
            return;
        };
        let url = url.to_string();
        let index = *self.indices.entry(url.clone()).or_insert_with(|| {
            self.entries.push((url.clone(), url));
            self.entries.len() - 1
        });
        self.active = Some(Label {
            index,
            text: String::new(),
            characters: 0,
        });
    }

    pub(super) fn text(&mut self, text: &str) {
        let Some(label) = &mut self.active else {
            return;
        };
        for character in text.chars().take(500 - label.characters) {
            label.text.push(if character.is_control() {
                ' '
            } else {
                character
            });
            label.characters += 1;
        }
    }

    pub(super) fn end(&mut self) {
        if let Some(label) = self.active.take()
            && !label.text.is_empty()
        {
            self.entries[label.index].0 = label.text;
        }
    }

    pub(super) fn finish(self) -> Vec<(String, String)> {
        self.entries
    }
}
