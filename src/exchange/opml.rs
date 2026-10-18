//! Typed OPML 2.0 decoding and export of active subscriptions.

use std::{collections::BTreeMap, path::Path};

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{source::discovery::Candidate, store::Store};

#[derive(Deserialize, Serialize)]
#[serde(rename = "opml")]
struct Document {
    #[serde(rename = "@version", default)]
    version: String,
    body: Body,
}

#[derive(Deserialize, Serialize)]
struct Body {
    #[serde(rename = "outline", default)]
    outlines: Vec<Outline>,
}

#[derive(Deserialize, Serialize)]
struct Outline {
    #[serde(rename = "@text", default)]
    text: String,
    #[serde(
        rename = "@title",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    title: String,
    #[serde(
        rename = "@xmlUrl",
        alias = "@xmlurl",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    url: Option<String>,
    #[serde(
        rename = "outline",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    children: Vec<Outline>,
}

/// Decode nested OPML, validating every URL and deduplicating normalized URLs.
///
/// # Errors
/// Returns malformed/deep XML, invalid URLs or resource-limit errors.
pub fn decode(xml: &str) -> Result<Vec<Candidate>> {
    validate_depth(xml)?;
    let document: Document = quick_xml::de::from_str(xml)?;
    ensure!(
        document.version.is_empty()
            || matches!(document.version.as_str(), "1.0" | "1.1" | "2.0"),
        "Unsupported OPML version"
    );
    let mut pending = document.body.outlines;
    let mut feeds = BTreeMap::new();
    while let Some(outline) = pending.pop() {
        pending.extend(outline.children);
        if let Some(address) = outline.url {
            let url = crate::source::feed_url(&address)?.to_string();
            let title = if outline.title.is_empty() {
                outline.text
            } else {
                outline.title
            };
            feeds.entry(url.clone()).or_insert(Candidate {
                title: crate::content::label(&title, 200),
                url,
            });
        }
        ensure!(feeds.len() <= 5000, "OPML exceeds 5,000 subscriptions");
    }
    Ok(feeds.into_values().collect())
}

/// Export active subscriptions as OPML without overwriting an existing file.
///
/// # Errors
/// Returns SQLite, XML encoding or filesystem errors.
pub fn export(store: &Store, path: &Path) -> Result<()> {
    let outlines = store
        .feeds()?
        .into_iter()
        .map(|feed| Outline {
            text: feed.title,
            title: String::new(),
            url: Some(feed.url),
            children: Vec::new(),
        })
        .collect();
    let document = Document {
        version: "2.0".into(),
        body: Body { outlines },
    };
    let mut xml = String::new();
    let mut serializer = quick_xml::se::Serializer::new(&mut xml);
    serializer.indent(' ', 4);
    document.serialize(serializer)?;
    super::atomic_file(path, |writer| {
        writer.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")?;
        writer.write_all(xml.as_bytes())?;
        Ok(())
    })
}

fn validate_depth(xml: &str) -> Result<()> {
    ensure!(xml.len() <= 4 * 1024 * 1024, "OPML exceeds 4 MiB");
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut depth = 0_usize;
    let mut nodes = 0_usize;
    loop {
        match reader.read_event()? {
            quick_xml::events::Event::Start(_) => {
                depth += 1;
                nodes += 1;
            }
            quick_xml::events::Event::End(_) => depth = depth.saturating_sub(1),
            quick_xml::events::Event::Empty(_) => nodes += 1,
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
        ensure!(
            depth <= 64 && nodes <= 20_000,
            "OPML structure exceeds safe limits"
        );
    }
    Ok(())
}
