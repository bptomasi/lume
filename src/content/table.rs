//! HTML5gum supplies table boundaries; this adapter retains readable cells.
//!
//! Rows are not padded to their widest cell, preventing quadratic output for
//! sparse/wide tables. Nested tables flatten inside the current outer cell.

#[derive(Default)]
pub(super) struct Table {
    rows: Vec<Vec<String>>,
    row: Vec<String>,
    cell: String,
    nested: usize,
    header: bool,
    link: Option<String>,
    bold_depth: usize,
    cell_bold: bool,
    cell_plain: bool,
    row_bold: Option<bool>,
}

impl Table {
    pub(super) fn tag(&mut self, name: &str, closing: bool) {
        if name == "table" {
            if closing {
                self.nested = self.nested.saturating_sub(1);
            } else {
                self.nested += 1;
            }
            return;
        }
        if self.nested > 0 {
            if closing && matches!(name, "td" | "th" | "tr") {
                self.cell.push(' ');
            }
            return;
        }
        match (name, closing) {
            ("tr", true) => self.finish_row(),
            ("td" | "th", true) => self.finish_cell(),
            ("th", false) if self.rows.is_empty() => self.header = true,
            ("thead", false) if self.rows.is_empty() => self.header = true,
            ("strong" | "b", false) => self.bold_depth += 1,
            ("strong" | "b", true) => {
                self.bold_depth = self.bold_depth.saturating_sub(1);
            }
            ("br", false) => self.cell.push_str(" / "),
            ("a", true) => self.close_link(),
            _ => {}
        }
    }

    pub(super) fn text(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.cell_bold |= self.bold_depth > 0;
            self.cell_plain |= self.bold_depth == 0;
        }
        self.cell.push_str(text);
    }

    pub(super) fn nested(&self) -> bool {
        self.nested > 0
    }

    pub(super) fn begin_link(&mut self, address: Option<String>) {
        self.close_link();
        if address.is_some() {
            self.cell.push('[');
        }
        self.link = address;
    }

    fn close_link(&mut self) {
        if let Some(address) = self.link.take() {
            self.cell.push_str(&format!("](<{address}>)"));
        }
    }

    fn finish_cell(&mut self) {
        self.close_link();
        let cell = self.cell.split_whitespace().collect::<Vec<_>>().join(" ");
        let bold = self.cell_bold && !self.cell_plain;
        self.row_bold = Some(self.row_bold.unwrap_or(true) && bold);
        self.row.push(cell.replace('|', "\\|"));
        self.cell.clear();
        self.cell_bold = false;
        self.cell_plain = false;
    }

    fn finish_row(&mut self) {
        if !self.cell.trim().is_empty() {
            self.finish_cell();
        }
        if !self.row.is_empty() {
            // Some publishers use fully bold td cells instead of th/thead.
            // Promote only that explicit visual convention, never plain data.
            if self.rows.is_empty() && self.row_bold == Some(true) {
                self.header = true;
            }
            self.rows.push(std::mem::take(&mut self.row));
        }
        self.row_bold = None;
    }

    pub(super) fn markdown(mut self) -> String {
        self.finish_row();
        let columns = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        if columns == 0 {
            return String::new();
        }
        if columns > 64 {
            return self
                .rows
                .iter()
                .map(|row| row.join(" | "))
                .collect::<Vec<_>>()
                .join("\n");
        }
        let mut output = String::new();
        if !self.header {
            output
                .push_str(&format!("| {} |\n", vec![" "; columns].join(" | ")));
            output.push_str(&format!(
                "| {} |\n",
                vec!["---"; columns].join(" | ")
            ));
        }
        if self.header {
            self.rows[0].resize(columns, String::new());
        }
        for (index, row) in self.rows.iter().enumerate() {
            output.push_str(&format!("| {} |\n", row.join(" | ")));
            if index == 0 && self.header {
                output.push_str(&format!(
                    "| {} |\n",
                    vec!["---"; columns].join(" | ")
                ));
            }
        }
        output
    }
}
