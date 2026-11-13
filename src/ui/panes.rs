//! Visible-row rendering for navigation, headlines and reading.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};

use crate::{
    app::{App, Focus},
    content,
};

use super::theme::*;

pub(super) fn library(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let block = panel(" 01  Library ".into(), app.focus == Focus::Library);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let inner = super::pane_search::draw(frame, app, Focus::Library, inner);
    super::navigation::draw(frame, app.navigation(), inner);
}

pub(super) fn articles(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let title = format!(" 02  Articles · {} ", app.navigation().page_total);
    let block = panel(title, app.focus == Focus::Articles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let inner = super::pane_search::draw(frame, app, Focus::Articles, inner);
    if app.navigation().visible.is_empty() {
        empty_articles(frame, app, inner);
        return;
    }
    let (first, capacity, row_height) =
        super::hit::article_window(app, inner.height);
    for (row, position) in (first..app.navigation().visible.len())
        .take(capacity)
        .enumerate()
    {
        let rect = Rect::new(
            inner.x,
            inner.y + row as u16 * row_height,
            inner.width,
            row_height.min(inner.height),
        );
        headline(frame, app, position, rect);
    }
}

fn headline(frame: &mut Frame<'_>, app: &App, position: usize, area: Rect) {
    let article =
        &app.navigation().library.articles[app.navigation().visible[position]];
    let selected = position == app.navigation().selected;
    let color = if selected {
        ACCENT
    } else if article.unread {
        TEXT
    } else {
        MUTED
    };
    if selected {
        frame.render_widget(Block::new().style(Style::new().bg(RAISED)), area);
    }
    let labels = app.headline_labels(article, area.width);
    let mut lines = headline_lines(article, labels, color);
    if app.navigation().marked.contains(&article.id) {
        lines[0]
            .spans
            .insert(0, Span::styled("✓", Style::new().fg(AMBER)));
    }
    if app.workspace.compact {
        lines.remove(1);
        lines.truncate(2);
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn headline_lines(
    article: &crate::model::Article,
    labels: crate::app::HeadlineLabels,
    color: ratatui::style::Color,
) -> Vec<Line<'static>> {
    let marker = if article.starred {
        "★"
    } else if article.unread {
        "●"
    } else {
        "·"
    };
    let title = Line::from(vec![
        Span::styled(
            format!(" {marker} "),
            Style::new().fg(if article.starred { AMBER } else { color }),
        ),
        Span::styled(
            labels.first,
            Style::new().fg(color).add_modifier(Modifier::BOLD),
        ),
    ]);
    let meta = format!("   {}  ·  {}", article.feed, labels.date);
    vec![
        title,
        Line::styled(format!("   {}", labels.second), Style::new().fg(color)),
        Line::styled(meta, Style::new().fg(FAINT)),
        Line::raw(""),
    ]
}

fn empty_articles(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let text = if *app.navigation().view == crate::model::View::Trash
        && app.navigation().search.is_empty()
    {
        EMPTY_TRASH
    } else if app
        .navigation()
        .navigation
        .iter()
        .any(|item| item.count > 0)
    {
        EMPTY_VIEW
    } else {
        EMPTY_LIBRARY
    };
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::new().fg(MUTED))
            .wrap(Wrap { trim: false }),
        area,
    );
}

pub(super) fn reader(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let reading = app.reader();
    let navigation = app.navigation();
    let title = if app.workspace.fullscreen {
        " Article · FULLSCREEN · Esc / F11 exit "
    } else {
        " 03  Reader "
    };
    let block = panel(title.into(), app.focus == Focus::Reader);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if navigation.navigation_loading && reading.displayed_article.is_none() {
        frame.render_widget(Paragraph::new("Loading article…"), inner);
        return;
    }
    let Some(article) = reading.displayed_article else {
        let message = if *navigation.view == crate::model::View::Trash
            && navigation.search.is_empty()
        {
            EMPTY_TRASH
        } else {
            EMPTY_READER
        };
        frame.render_widget(
            Paragraph::new(message).style(Style::new().fg(MUTED)),
            inner,
        );
        return;
    };
    let column = super::workspace_reader_content(area, &app.workspace);
    let header =
        Rect::new(column.x, inner.y, column.width, 3.min(inner.height));
    frame.render_widget(
        Paragraph::new(article.title.as_str())
            .style(Style::new().fg(TEXT).add_modifier(Modifier::BOLD))
            .wrap(Wrap { trim: true }),
        header,
    );
    reader_metadata(frame, app, column, inner);
    reader_body(frame, app, column, inner);
}

const EMPTY_LIBRARY: &str = "
  Your next idea starts here.

  :feed https://…
  Subscribe to a feed.

  r  Collect subscriptions
  ?  Explore the shortcuts

  RSS · Atom · JSON Feed";

const EMPTY_TRASH: &str = "
  Trash is empty.

  D / Delete moves a post here.
  :restore brings it back.
  u undoes a personal change.";

const EMPTY_VIEW: &str = "
  A little breathing room.

  No articles in this view.
  Esc clears the search.

  h  Explore your library";

const EMPTY_READER: &str = "
  A quiet place for your curiosity.

  Select an article to begin.";

fn reader_metadata(
    frame: &mut Frame<'_>,
    app: &App,
    column: Rect,
    inner: Rect,
) {
    let reading = app.reader();
    let Some(article) = reading.displayed_article else {
        return;
    };
    let availability = match reading.body_kind {
        content::BodyKind::Text => {
            format!("{} min", reading.reading_minutes)
        }
        content::BodyKind::ReferencesOnly => "links / media only".into(),
        content::BodyKind::Missing => "no feed text".into(),
    };
    let meta = format!(
        "{} · {} · {} · {} notes{}{}",
        match reading.content_source {
            crate::model::ContentSource::Feed => match reading.feed_origin {
                crate::model::FeedOrigin::Summary => "Summary from feed",
                _ => "Feed",
            },
            crate::model::ContentSource::Full => "Full article cached",
        },
        availability,
        article.feed,
        reading.comments.len(),
        if article.queued { " · read next" } else { "" },
        if *app.navigation().view == crate::model::View::Trash {
            " · Trash · :restore"
        } else {
            ""
        },
    );
    frame.render_widget(
        Paragraph::new(meta).style(Style::new().fg(AMBER)),
        Rect::new(column.x, inner.y + 3, column.width, 1),
    );
    article_fetch_status(frame, app, column, inner);
}

fn article_fetch_status(
    frame: &mut Frame<'_>,
    app: &App,
    column: Rect,
    inner: Rect,
) {
    let reading = app.reader();
    let Some(article) = reading.displayed_article else {
        return;
    };
    let message: std::borrow::Cow<'_, str> = if app.reader_stale() {
        "Previous article · loading selection… · article actions paused".into()
    } else if app.article_fetching.as_ref().is_some_and(|target| {
        target.article == article.id && target.url == article.url
    }) {
        "Fetching article… · reading remains available".into()
    } else if let Some(cache) = &reading.document
        && cache.attempt_url == article.url
        && !cache.error.is_empty()
    {
        format!("Fetch failed: {} · :fetch retries", cache.error).into()
    } else if !reading.article_warning.is_empty() {
        reading.article_warning.into()
    } else if reading.reader_tools.index.limited {
        "Structure index limited · article text remains available".into()
    } else {
        ":content switches Feed / Full · :fetch retrieves the original".into()
    };
    frame.render_widget(
        Paragraph::new(message).style(Style::new().fg(MUTED)),
        Rect::new(column.x, inner.y + 4, column.width, 1),
    );
}

fn reader_body(frame: &mut Frame<'_>, app: &App, column: Rect, inner: Rect) {
    let reading = app.reader();
    if app.article_preparing() && !app.reader_stale() {
        frame.render_widget(
            Paragraph::new("Preparing article…").style(Style::new().fg(MUTED)),
            column,
        );
        return;
    }
    super::reader_text::backgrounds(frame, app, column);
    let selected = reading.selected_range();
    let lines = reading
        .reader_styled
        .iter()
        .enumerate()
        .skip(reading.scroll)
        .take(column.height as usize)
        .map(|(index, line)| {
            super::reader_text::row(
                app,
                index,
                line,
                selected.as_ref(),
                column.width as usize,
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lines).style(Style::new().fg(TEXT)),
        column,
    );
    super::images::draw(frame, app, column);
    super::note_preview::markers(frame, app, column);
    reader_progress(frame, app, inner);
}

fn reader_progress(frame: &mut Frame<'_>, app: &App, inner: Rect) {
    let reading = app.reader();
    let percent =
        cursor_percent(reading.reader_cursor, reading.reader_lines.len());
    let text = if app.workspace.fullscreen {
        format!(
            " {percent}% · Esc / F11 exit · {}",
            content::label(&app.status, 160),
        )
    } else {
        format!(
            " {percent}% · v select · y copy · s highlight · Tab panels · Z fullscreen"
        )
    };
    frame.render_widget(
        Paragraph::new(text).style(Style::new().fg(FAINT)),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn cursor_percent(cursor: usize, rows: usize) -> usize {
    match rows {
        0 => 0,
        1 => 100,
        _ => (cursor.min(rows - 1) as u128 * 100 / (rows - 1) as u128) as usize,
    }
}

#[cfg(test)]
mod tests {
    use super::cursor_percent;

    #[test]
    fn progress_tracks_the_cursor_independently_of_viewport_height() {
        assert_eq!(cursor_percent(0, 101), 0);
        assert_eq!(cursor_percent(50, 101), 50);
        assert_eq!(cursor_percent(100, 101), 100);
        assert_eq!(cursor_percent(200, 101), 100);
        assert_eq!(cursor_percent(0, 0), 0);
        assert_eq!(cursor_percent(0, 1), 100);
        assert_eq!(cursor_percent(0, 8), 0);
        assert_eq!(cursor_percent(7, 8), 100);
        assert_eq!(cursor_percent(usize::MAX / 2, usize::MAX), 50);
    }
}
