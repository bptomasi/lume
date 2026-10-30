//! Prepared page text uses the qualified extractor, Markdown and cursor owners.

use std::sync::Arc;

use anyhow::{Context, Result, ensure};

use crate::{
    reader::{Rendered, index::DocumentIndex},
    store::Store,
};

pub use crate::model::WebDocument;

pub(super) enum Loaded {
    Page(Box<WebPage>),
    Browser(String),
}

/// Width-specific linked reading; no article ID or mutation authority is present.
pub struct WebPage {
    /// Captured source.
    pub document: Arc<WebDocument>,
    /// Original navigation target, including fragments omitted from cache keys.
    pub(super) browser_url: String,
    /// Recoverable navigation issue; readable page content remains available.
    pub(super) notice: Option<String>,
    /// Cached styled text and original byte correspondence.
    pub rendered: Rendered,
    /// Current readable-text cursor byte.
    pub cursor: usize,
    /// First visible display row.
    pub scroll: usize,
    /// Optional visual selection anchor.
    pub selection: Option<usize>,
    pub(super) linewise: bool,
    /// Width used for cached layout.
    pub width: usize,
    pub(super) options: crate::reader::RenderOptions,
    pub(crate) columns: Vec<crate::reader::viewport::RowIndex>,
    pub(crate) rows: Vec<String>,
    horizontal: std::collections::BTreeMap<usize, usize>,
    pub(super) index: DocumentIndex,
    pub(super) words: crate::reader::motion::WordIndex,
    pub(super) row_goal: Option<usize>,
    pub(super) back: Vec<usize>,
    pub(super) forward: Vec<usize>,
}

impl WebPage {
    pub(super) fn prepare(
        document: Arc<WebDocument>,
        width: usize,
        anchor: usize,
        options: crate::reader::RenderOptions,
    ) -> Self {
        let source = &document.body;
        let mut rendered =
            crate::reader::render_with_options(source, width, options);
        let base = crate::source::feed_url(&document.url).ok();
        super::super::images::resolve_urls(&mut rendered.images, base.as_ref());
        let index = DocumentIndex::parse_with_options(source, options);
        let cursor = crate::reader::to_display(
            &rendered.mappings,
            anchor..anchor.saturating_add(1),
        )
        .first()
        .map_or(0, |range| range.start);
        let words = crate::reader::motion::WordIndex::new(&rendered.text);
        let rows = rendered
            .lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let columns = index_rows(&rows, &rendered);
        Self {
            browser_url: document.url.clone(),
            notice: None,
            document,
            rendered,
            cursor,
            scroll: 0,
            selection: None,
            linewise: false,
            index,
            words,
            row_goal: None,
            back: Vec::new(),
            forward: Vec::new(),
            width,
            options,
            rows,
            columns,
            horizontal: std::collections::BTreeMap::new(),
        }
    }

    /// Display row containing the cursor, including empty rows.
    pub fn cursor_row(&self) -> usize {
        self.rendered
            .text_ranges
            .partition_point(|range| range.end < self.cursor)
            .min(self.rendered.lines.len().saturating_sub(1))
    }

    pub(super) fn source_anchor(&self) -> usize {
        let row = self.cursor_row();
        // Generated image rows and paragraph separators have no text mapping.
        // Clamp to the preceding readable grapheme before projecting an anchor.
        let cursor = self
            .rendered
            .text_ranges
            .iter()
            .take(row.saturating_add(1))
            .rfind(|range| !range.is_empty())
            .map_or(self.cursor, |range| {
                self.cursor.min(crate::reader::cursor::previous(
                    &self.rendered.text,
                    range.end,
                ))
            });
        crate::reader::to_source(
            &self.rendered.mappings,
            cursor..crate::reader::cursor::next(&self.rendered.text, cursor),
        )
        .map_or_else(
            || self.rendered.ranges.get(row).map_or(0, |range| range.start),
            |range| range.start,
        )
    }

    /// Supported link at the precise cursor occurrence.
    pub fn cursor_link(&self) -> Option<&str> {
        let range = crate::reader::to_source(
            &self.rendered.mappings,
            self.cursor..self.cursor.saturating_add(1),
        )?;
        self.index
            .navigation
            .link_at(range.start)
            .map(|link| link.destination.as_str())
    }

    pub(super) fn reveal(&mut self, height: usize) {
        let row = self.cursor_row();
        if row < self.scroll {
            self.scroll = row;
        }
        if row >= self.scroll.saturating_add(height.max(1)) {
            self.scroll = row.saturating_sub(height.max(1) - 1);
        }
        if let Some(Some(group)) = self.rendered.horizontal_groups.get(row)
            && let Some(range) = self.rendered.text_ranges.get(row)
        {
            let column = self.columns[row].column(
                &self.rows[row],
                self.cursor.saturating_sub(range.start),
            );
            let offset = self.horizontal.entry(*group).or_default();
            if column < *offset {
                *offset = column;
            }
            if column >= offset.saturating_add(self.width) {
                *offset = column.saturating_sub(self.width.saturating_sub(1));
            }
        }
    }

    pub(super) fn inherit_view(&mut self, previous: &mut Self) {
        self.browser_url.clone_from(&previous.browser_url);
        self.notice.clone_from(&previous.notice);
        self.seek_source(previous.source_anchor());
        self.selection = previous.selection.and_then(|anchor| {
            let source = crate::reader::to_source(
                &previous.rendered.mappings,
                anchor..anchor.saturating_add(1),
            )?;
            crate::reader::to_display(&self.rendered.mappings, source)
                .first()
                .map(|range| range.start)
        });
        self.linewise = previous.linewise;
        self.back = std::mem::take(&mut previous.back);
        self.forward = std::mem::take(&mut previous.forward);
    }

    fn source_position(&self, anchor: usize) -> Option<usize> {
        let display = crate::reader::to_display(
            &self.rendered.mappings,
            anchor..anchor.saturating_add(1),
        );
        display
            .first()
            .map(|range| range.start)
            .or_else(|| {
                self.rendered.images.iter().find_map(|slot| {
                    slot.source
                        .contains(&anchor)
                        .then(|| self.rendered.text_ranges[slot.caption].start)
                })
            })
            .or_else(|| {
                // Heading markers/HTML anchors are intentionally invisible.
                self.rendered
                    .mappings
                    .iter()
                    .filter(|mapping| {
                        mapping.source.start >= anchor
                            && !mapping.display.is_empty()
                    })
                    .min_by_key(|mapping| mapping.source.start)
                    .map(|mapping| mapping.display.start)
            })
    }

    pub(super) fn seek_source(&mut self, anchor: usize) {
        self.cursor = self.source_position(anchor).unwrap_or(0);
    }

    pub(super) fn jump_fragment(&mut self, fragment: &str) -> Result<()> {
        let anchor = self
            .index
            .navigation
            .anchor(fragment)
            .context("Section is absent from this page")?;
        self.cursor = self
            .source_position(anchor)
            .context("Section has no readable text")?;
        Ok(())
    }

    /// Exact readable selection, including display rows for visual-line mode.
    pub fn selected_range(&self) -> Option<std::ops::Range<usize>> {
        let anchor = self.selection?;
        let start = anchor.min(self.cursor);
        let end = crate::reader::cursor::next(
            &self.rendered.text,
            anchor.max(self.cursor),
        );
        if !self.linewise {
            return Some(start..end);
        }
        let rows = &self.rendered.text_ranges;
        let first = rows
            .partition_point(|range| range.end < start)
            .min(rows.len().saturating_sub(1));
        let last = rows
            .partition_point(|range| range.end < anchor.max(self.cursor))
            .min(rows.len().saturating_sub(1));
        Some(rows.get(first)?.start..rows.get(last)?.end)
    }

    /// Per-block horizontal viewport in terminal cells.
    pub fn horizontal(&self, row: usize) -> usize {
        self.rendered
            .horizontal_groups
            .get(row)
            .copied()
            .flatten()
            .and_then(|group| self.horizontal.get(&group))
            .copied()
            .unwrap_or(0)
    }
}

fn index_rows(
    rows: &[String],
    rendered: &Rendered,
) -> Vec<crate::reader::viewport::RowIndex> {
    rows.iter()
        .zip(&rendered.lines)
        .zip(&rendered.insets)
        .map(|((text, line), inset)| {
            crate::reader::viewport::RowIndex::inset(text, line, *inset)
        })
        .collect()
}

pub(super) fn load(
    connection: &mut Option<Store>,
    path: &std::path::Path,
    address: &str,
    online: bool,
    width: usize,
    fragment: Option<String>,
    options: crate::reader::RenderOptions,
) -> Result<Loaded> {
    let document = match fetch(connection, path, address, online)? {
        crate::source::article::FetchedPage::Article(document) => document,
        crate::source::article::FetchedPage::Browser(address) => {
            let mut url = crate::source::link_url(&address)?;
            if url.fragment().is_none() {
                url.set_fragment(fragment.as_deref());
            }
            return Ok(Loaded::Browser(url.to_string()));
        }
    };
    let mut target = crate::source::link_url(&document.final_url)?;
    if fragment.is_some() {
        target.set_fragment(fragment.as_deref());
    }
    let fragment = target.fragment().map(str::to_owned);
    let browser_url = target.to_string();
    target.set_fragment(None);
    let url = target.to_string();
    let body = document.body;
    let title = body
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .map_or_else(|| url.clone(), |title| crate::content::label(title, 160));
    let document = Arc::new(WebDocument { url, title, body });
    let mut page = WebPage::prepare(document, width, 0, options);
    page.browser_url = browser_url;
    if let Some(fragment) = fragment
        && page.jump_fragment(&fragment).is_err()
    {
        page.notice = Some(
            "Section unavailable · showing page start · o opens the original"
                .into(),
        );
    }
    Ok(Loaded::Page(Box::new(page)))
}

fn fetch(
    connection: &mut Option<Store>,
    path: &std::path::Path,
    address: &str,
    online: bool,
) -> Result<crate::source::article::FetchedPage> {
    if connection.is_none() {
        *connection = Some(Store::open(path)?);
    }
    let store = connection
        .as_ref()
        .context("Linked-page connection unavailable")?;
    let url = crate::source::link_url(address)?;
    let cached = store.cached_page_body(url.as_str())?;
    if let Some(body) = cached {
        return Ok(crate::source::article::FetchedPage::Article(
            crate::model::ExtractedDocument {
                final_url: url.to_string(),
                body,
            },
        ));
    }
    ensure!(online, "Page is not cached · network is disabled");
    crate::source::article::fetch_page(
        &crate::source::agent(),
        url.as_str(),
        "",
    )
}
