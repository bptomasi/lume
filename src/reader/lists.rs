//! List markers are emitted with content, after any loose-paragraph boundary.

#[derive(Default)]
pub(super) struct Lists {
    stack: Vec<List>,
}

struct List {
    next: Option<u64>,
    indent: usize,
    content_indent: usize,
    marker: Option<String>,
}

impl Lists {
    pub(super) fn begin(&mut self, start: Option<u64>) {
        let indent = self.indent();
        self.stack.push(List {
            next: start,
            indent,
            content_indent: indent,
            marker: None,
        });
    }

    pub(super) fn end(&mut self) {
        self.stack.pop();
    }

    pub(super) fn item(&mut self) {
        let Some(list) = self.stack.last_mut() else {
            return;
        };
        let marker = list
            .next
            .map_or_else(|| "• ".to_owned(), |number| format!("{number}. "));
        list.next = list.next.map(|number| number.saturating_add(1));
        list.content_indent = list.indent + marker.chars().count();
        list.marker = Some(marker);
    }

    pub(super) fn pending(&self) -> bool {
        self.stack.last().is_some_and(|list| list.marker.is_some())
    }

    pub(super) fn prefix(&mut self) -> String {
        let Some(list) = self.stack.last_mut() else {
            return String::new();
        };
        match list.marker.take() {
            Some(marker) => format!("{}{marker}", " ".repeat(list.indent)),
            None => " ".repeat(list.content_indent),
        }
    }

    pub(super) fn indent(&self) -> usize {
        self.stack.last().map_or(0, |list| list.content_indent)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}
