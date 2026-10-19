//! Preserve publisher image references without loading remote assets.

use super::Fragment;

impl Fragment {
    pub(super) fn image(&mut self, tag: &html5gum::StartTag<()>) {
        let address = [b"data-src".as_slice(), b"src".as_slice()]
            .into_iter()
            .filter_map(|name| tag.attributes.get(name))
            .find_map(|attribute| self.link_url(&attribute.value));
        let Some(address) = address else {
            return;
        };
        let alt = tag
            .attributes
            .get(b"alt".as_slice())
            .map(|attribute| String::from_utf8_lossy(&attribute.value))
            .unwrap_or_default();
        let alt = super::label(&alt, 300)
            .replace('\\', "\\\\")
            .replace('[', "\\[")
            .replace(']', "\\]");
        let markdown = format!("![{alt}](<{address}>)");
        if let Some(table) = &mut self.table {
            table.text(&markdown);
        } else {
            self.text.push_str(&markdown);
        }
    }
}
