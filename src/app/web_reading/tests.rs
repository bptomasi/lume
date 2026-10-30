//! Reflow shares width-independent structure and reindexes formula policy changes.

use std::time::{Duration, Instant};

use super::*;
use crate::store::Store;

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.web_pending() {
        app.poll_web()?;
        ensure!(Instant::now() < deadline, "Web worker timed out");
        std::thread::yield_now();
    }
    Ok(())
}

#[test]
fn reflow_reuses_the_index_until_formula_semantics_change() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let document = WebDocument {
        url: "https://example.org/page".into(),
        title: "Test".into(),
        body: "# $x^2$\n\nReadable source with a [link](#x2).".into(),
    };
    store.save_web_document(&document)?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(ratatui::layout::Rect::new(0, 0, 120, 40));
    app.open_web_page(&document.url)?;
    settle(&mut app)?;
    let index = app.web_page().context("Page")?.index.clone();
    assert_eq!(index.headings[0].title, "x²");
    app.resize_workspace(ratatui::layout::Rect::new(0, 0, 60, 40));
    app.poll_web()?;
    settle(&mut app)?;
    assert!(Arc::ptr_eq(
        &index,
        &app.web_page().context("Reflow")?.index
    ));
    app.set_formula_rendering("off")?;
    app.poll_web()?;
    settle(&mut app)?;
    let page = app.web_page().context("Changed policy")?;
    assert!(!Arc::ptr_eq(&index, &page.index));
    assert_eq!(page.index.headings[0].title, "$x^2$");
    Ok(())
}

#[test]
fn contiguous_unicode_wraps_place_cursor_and_line_selection_on_the_new_row() {
    let document = Arc::new(WebDocument {
        url: "https://example.org/unicode".into(),
        title: "Unicode".into(),
        body: "界界界界界界界界界界界界".into(),
    });
    let mut page = WebPage::prepare(document, 4, Default::default(), None);
    page.cursor = page.rendered.text_ranges[1].start;
    assert_eq!(page.cursor_row(), 1);
    page.selection = Some(page.cursor);
    page.linewise = true;
    assert_eq!(
        page.selected_range(),
        Some(page.rendered.text_ranges[1].clone())
    );
}
