//! Configuration layout uses toml_edit's tree, never a second TOML parser.
//!
//! Generated records become tables; wide scalar arrays use one item per row.
//! Existing inline containers with comments retain their layout because the
//! library's table conversion clears child decoration. Comments are inspected
//! only in parsed trivia, so a hash inside a quoted value is ordinary data.

use anyhow::{Result, ensure};
use toml_edit::{
    Array, Decor, DocumentMut, InlineTable, Item, RawString, Table, Value,
};
use toml_edit::{
    visit::{self, Visit},
    visit_mut::{self, VisitMut},
};

pub(super) fn generated(config: &super::Configuration) -> Result<DocumentMut> {
    let mut document = toml_edit::ser::to_document(config)?;
    Layout.visit_document_mut(&mut document);
    Ok(document)
}

pub(super) fn format(text: &str) -> Result<String> {
    let before = super::decode(text)?;
    let mut document = text.parse::<DocumentMut>()?;
    Layout.visit_document_mut(&mut document);
    let formatted = document.to_string();
    ensure!(
        super::decode(&formatted)? == before,
        "Formatting changed configuration values; file was not replaced"
    );
    Ok(formatted)
}

struct Layout;

impl VisitMut for Layout {
    fn visit_item_mut(&mut self, item: &mut Item) {
        if let Item::Value(value) = item {
            let mut comments = Comments::default();
            comments.visit_value(value);
            if !comments.0 {
                let value = std::mem::take(item);
                *item = match value.into_table() {
                    Ok(table) => Item::Table(table),
                    Err(value) => match value.into_array_of_tables() {
                        Ok(tables) => Item::ArrayOfTables(tables),
                        Err(value) => value,
                    },
                };
            }
        }
        visit_mut::visit_item_mut(self, item);
    }

    fn visit_table_mut(&mut self, table: &mut Table) {
        for (mut key, item) in table.iter_mut() {
            let suffix = if item.is_value() { " " } else { "" };
            normalize_spacing(key.leaf_decor_mut(), "", suffix);
            if let Some(value) = item.as_value_mut() {
                normalize_spacing(value.decor_mut(), " ", "");
            }
        }
        visit_mut::visit_table_mut(self, table);
        for (mut key, item) in table.iter_mut() {
            if !item.is_value() {
                normalize_spacing(key.leaf_decor_mut(), "", "");
            }
            let prefix = key
                .leaf_decor()
                .prefix()
                .and_then(RawString::as_str)
                .unwrap_or("")
                .to_owned();
            if !prefix.contains('#') {
                continue;
            }
            let header = match item {
                Item::Table(table) => Some(table),
                Item::ArrayOfTables(tables) => tables.get_mut(0),
                _ => None,
            };
            if let Some(table) = header {
                table.decor_mut().set_prefix(prefix);
                key.leaf_decor_mut().set_prefix("");
            }
        }
    }

    fn visit_array_mut(&mut self, array: &mut Array) {
        let mut comments = Comments::default();
        comments.visit_array(array);
        if !comments.0 && array.to_string().chars().count() > 60 {
            for value in array.iter_mut() {
                value.decor_mut().set_prefix("\n    ");
                value.decor_mut().set_suffix("");
            }
            array.set_trailing_comma(true);
            array.set_trailing("\n");
        }
        visit_mut::visit_array_mut(self, array);
    }
}

#[derive(Default)]
struct Comments(bool);

impl<'doc> Visit<'doc> for Comments {
    fn visit_value(&mut self, value: &'doc Value) {
        self.0 |= decorated(value.decor());
        visit::visit_value(self, value);
    }

    fn visit_array(&mut self, array: &'doc Array) {
        self.0 |= commented(array.trailing());
        visit::visit_array(self, array);
    }

    fn visit_inline_table(&mut self, table: &'doc InlineTable) {
        self.0 |= commented(table.trailing());
        for (name, _) in table.iter() {
            if let Some(key) = table.key(name) {
                self.0 |= decorated(key.leaf_decor())
                    || decorated(key.dotted_decor());
            }
        }
        visit::visit_inline_table(self, table);
    }
}

fn decorated(decor: &Decor) -> bool {
    decor.prefix().is_some_and(commented)
        || decor.suffix().is_some_and(commented)
}

fn normalize_spacing(decor: &mut Decor, prefix: &str, suffix: &str) {
    if !decor.prefix().is_some_and(commented) {
        decor.set_prefix(prefix);
    }
    if !decor.suffix().is_some_and(commented) {
        decor.set_suffix(suffix);
    }
}

fn commented(raw: &RawString) -> bool {
    raw.as_str().is_some_and(|text| text.contains('#'))
}
