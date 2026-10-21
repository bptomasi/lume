//! Source-bound links and explicit/heading anchors are indexed once per body.

use std::{collections::BTreeMap, ops::Range};

use pulldown_cmark::{Event, Tag};

/// A supported link occurrence, retaining its own source range.
#[derive(Clone, Debug)]
pub struct Link {
    /// Original Markdown link range, including its label and destination.
    pub range: Range<usize>,
    /// A local fragment or validated HTTP(S) address.
    pub destination: String,
}

/// Width-independent destinations; duplicate IDs retain their first location.
#[derive(Clone, Debug, Default)]
pub struct Navigation {
    links: Vec<Link>,
    anchors: BTreeMap<String, usize>,
    slugs: BTreeMap<String, usize>,
    pending: Vec<String>,
}

impl Navigation {
    pub(super) fn event(&mut self, event: &Event<'_>, range: Range<usize>) {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                if supported(dest_url) {
                    self.links.push(Link {
                        range,
                        destination: dest_url.to_string(),
                    });
                }
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                if let Some(id) = crate::content::anchors::from_html(html)
                    && let std::collections::btree_map::Entry::Vacant(entry) =
                        self.anchors.entry(id.clone())
                {
                    entry.insert(range.end);
                    self.pending.push(id);
                }
            }
            Event::Text(text) | Event::Code(text)
                if !text.trim().is_empty() =>
            {
                // The marker itself has no visible bytes. Bind it to the next
                // actual token so paragraph separators cannot receive focus.
                for id in self.pending.drain(..) {
                    self.anchors.insert(id, range.start);
                }
            }
            _ => {}
        }
    }

    pub(super) fn heading(&mut self, title: &str, offset: usize) {
        let slug = title
            .to_lowercase()
            .chars()
            .filter(|character| {
                character.is_alphanumeric()
                    || matches!(character, '_' | '-')
                    || character.is_whitespace()
            })
            .map(|character| {
                if character.is_whitespace() {
                    '-'
                } else {
                    character
                }
            })
            .collect::<String>();
        let count = self.slugs.entry(slug.clone()).or_default();
        let id = if *count == 0 {
            slug
        } else {
            format!("{slug}-{count}")
        };
        *count += 1;
        self.anchors.entry(id).or_insert(offset);
    }

    /// Find the occurrence containing an original source byte.
    pub fn link_at(&self, offset: usize) -> Option<&Link> {
        let end = self
            .links
            .partition_point(|link| link.range.start <= offset);
        self.links[..end]
            .last()
            .filter(|link| link.range.contains(&offset))
    }

    /// Resolve an encoded fragment; an empty fragment means article start.
    pub fn anchor(&self, fragment: &str) -> Option<usize> {
        if fragment.is_empty() {
            return Some(0);
        }
        // Reuse URL decoding while protecting literal form separators and '+'.
        let encoded = fragment.replace('+', "%2B").replace('&', "%26");
        let query = format!("id={encoded}");
        let (_, decoded) =
            url::form_urlencoded::parse(query.as_bytes()).next()?;
        self.anchors.get(decoded.as_ref()).copied()
    }
}

fn supported(destination: &str) -> bool {
    if destination.len() > 8192 || destination.chars().any(char::is_control) {
        return false;
    }
    destination.starts_with('#') || crate::source::link_url(destination).is_ok()
}
