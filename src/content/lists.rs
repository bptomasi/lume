//! HTML list state preserves CommonMark indentation across block boundaries.

#[derive(Default)]
pub(super) struct Lists {
    stack: Vec<List>,
}

struct List {
    next: Option<u64>,
    indent: usize,
    content_indent: usize,
    content_start: Option<usize>,
}

impl Lists {
    pub(super) fn begin(&mut self, start: Option<u64>) {
        // Limit generated whitespace for deeply nested, untrusted HTML.
        let indent = self.indent().min(64);
        self.stack.push(List {
            next: start,
            indent,
            content_indent: indent,
            content_start: None,
        });
    }

    pub(super) fn end(&mut self) {
        self.stack.pop();
    }

    pub(super) fn item(&mut self) -> String {
        let Some(list) = self.stack.last_mut() else {
            return "- ".into();
        };
        let marker = list
            .next
            .map_or_else(|| "- ".to_owned(), |number| format!("{number}. "));
        list.next = list.next.map(|number| number.saturating_add(1));
        list.content_indent = list.indent + marker.len();
        format!("{}{marker}", " ".repeat(list.indent))
    }

    pub(super) fn content(&mut self, start: usize) {
        if let Some(list) = self.stack.last_mut() {
            list.content_start = Some(start);
        }
    }

    pub(super) fn end_item(&mut self, trimmed_end: usize) -> Option<usize> {
        let list = self.stack.last_mut()?;
        let start = list.content_start.take()?;
        // Ordered spacers must remain in Markdown so CommonMark keeps the
        // author's subsequent numbering. The reader hides their empty rows.
        (list.next.is_none() && trimmed_end <= start)
            .then(|| start.saturating_sub(list.content_indent))
    }

    pub(super) fn pending(&self, trimmed_end: usize) -> bool {
        self.stack
            .last()
            .and_then(|list| list.content_start)
            .is_some_and(|start| trimmed_end <= start)
    }

    pub(super) fn indent(&self) -> usize {
        self.stack.last().map_or(0, |list| {
            if list.content_start.is_some() {
                list.content_indent
            } else {
                list.indent
            }
        })
    }
}
