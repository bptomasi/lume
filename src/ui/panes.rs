//! Visible-row rendering for navigation, headlines and reading.

use chrono::{DateTime, Utc};
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
    let rows = navigation_rows(app, inner.width);
    let selected_row = rows
        .iter()
        .position(|(_, index)| *index == Some(app.nav_selected))
        .unwrap_or(0);
    let offset = selected_row.saturating_sub(inner.height as usize / 2);
    let lines = rows
        .into_iter()
        .skip(offset)
        .take(inner.height as usize)
        .map(|(line, _)| line)
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), inner);
}

pub(super) fn navigation_rows(
    app: &App,
    width: u16,
) -> Vec<(Line<'static>, Option<usize>)> {
    let mut lines = Vec::new();
    let mut group = "";
    for (index, item) in app.navigation.iter().enumerate() {
        if item.group != group {
            group = item.group;
            lines.push((Line::raw(""), None));
            lines.push((
                Line::styled(format!("  {group}"), Style::new().fg(FAINT)),
                None,
            ));
        }
        let selected = index == app.nav_selected;
        let marker = if selected { "▎" } else { " " };
        let title =
            content::label(&item.title, width.saturating_sub(8) as usize);
        let padding =
            width.saturating_sub(title.chars().count() as u16 + 6) as usize;
        let text = format!(
            "{marker} {title}{}{:>3} ",
            " ".repeat(padding),
            item.count
        );
        let style = if selected {
            Style::new()
                .fg(ACCENT)
                .bg(RAISED)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(MUTED)
        };
        lines.push((Line::styled(text, style), Some(index)));
    }
    lines
}

pub(super) fn articles(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let title = format!(" 02  Articles · {} ", app.page_total);
    let block = panel(title, app.focus == Focus::Articles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if app.visible.is_empty() {
        empty_articles(frame, app, inner);
        return;
    }
    let (first, capacity, row_height) =
        super::hit::article_window(app, inner.height);
    for (row, position) in (first..app.visible.len()).take(capacity).enumerate()
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
    let article = &app.library.articles[app.visible[position]];
    let selected = position == app.selected;
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
    let mut lines = headline_lines(article, area.width, color);
    if app.marked.contains(&article.id) {
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
    width: u16,
    color: ratatui::style::Color,
) -> Vec<Line<'static>> {
    let marker = if article.starred {
        "★"
    } else if article.unread {
        "●"
    } else {
        "·"
    };
    let wrapped =
        textwrap::wrap(&article.title, width.saturating_sub(4).max(1) as usize);
    let first = wrapped.first().map_or("", |line| line.as_ref());
    let second = wrapped.get(1).map_or("", |line| line.as_ref());
    let title = Line::from(vec![
        Span::styled(
            format!(" {marker} "),
            Style::new().fg(if article.starred { AMBER } else { color }),
        ),
        Span::styled(
            first.to_owned(),
            Style::new().fg(color).add_modifier(Modifier::BOLD),
        ),
    ]);
    let meta =
        format!("   {}  ·  {}", article.feed, date_label(article.published));
    vec![
        title,
        Line::styled(format!("   {second}"), Style::new().fg(color)),
        Line::styled(meta, Style::new().fg(FAINT)),
        Line::raw(""),
    ]
}

fn date_label(published: i64) -> String {
    DateTime::<Utc>::from_timestamp(published, 0)
        .filter(|_| published != 0)
        .map_or_else(
            || "undated".into(),
            |date| date.format("%b %d").to_string(),
        )
}

fn empty_articles(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let text = if app.view == crate::model::View::Trash && app.search.is_empty()
    {
        EMPTY_TRASH
    } else if app.navigation.iter().any(|item| item.count > 0) {
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
    let title = if app.workspace.fullscreen {
        " Article · FULLSCREEN · Esc / F11 exit "
    } else {
        " 03  Reader "
    };
    let block = panel(title.into(), app.focus == Focus::Reader);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if app.navigation_loading {
        frame.render_widget(Paragraph::new("Loading article…"), inner);
        return;
    }
    let Some(article) = app.current() else {
        frame.render_widget(
            Paragraph::new(
                if app.view == crate::model::View::Trash
                    && app.search.is_empty()
                {
                    EMPTY_TRASH
                } else {
                    EMPTY_READER
                },
            )
            .style(Style::new().fg(MUTED)),
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
    let Some(article) = app.current() else {
        return;
    };
    let availability = match app.body_kind {
        content::BodyKind::Text => format!("{} min", app.reading_minutes),
        content::BodyKind::ReferencesOnly => "links / media only".into(),
        content::BodyKind::Missing => "no feed text".into(),
    };
    let meta = format!(
        "{} · {} · {} · {} notes{}{}",
        match app.content_source {
            crate::model::ContentSource::Feed => match app.feed_origin {
                crate::model::FeedOrigin::Summary => "Summary from feed",
                _ => "Feed",
            },
            crate::model::ContentSource::Full => "Full article cached",
        },
        availability,
        article.feed,
        app.comments.len(),
        if article.queued { " · read next" } else { "" },
        if app.view == crate::model::View::Trash {
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
    let Some(article) = app.current() else {
        return;
    };
    let message = if app.article_fetching.as_ref().is_some_and(|target| {
        target.article == article.id && target.url == article.url
    }) {
        "Fetching article… · reading remains available".into()
    } else if let Some(cache) = &app.document
        && cache.attempt_url == article.url
        && !cache.error.is_empty()
    {
        format!("Fetch failed: {} · :fetch retries", cache.error)
    } else if !app.article_warning.is_empty() {
        app.article_warning.clone()
    } else if app.reader_tools.index.limited {
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
    if app.article_preparing() {
        frame.render_widget(
            Paragraph::new("Preparing article…").style(Style::new().fg(MUTED)),
            column,
        );
        return;
    }
    let selected = app.selected_range();
    let lines = app
        .reader_styled
        .iter()
        .enumerate()
        .skip(app.scroll)
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
    super::note_preview::markers(frame, app, column);
    reader_progress(frame, app, column, inner);
}

fn reader_progress(
    frame: &mut Frame<'_>,
    app: &App,
    column: Rect,
    inner: Rect,
) {
    let percent = ((app.scroll + column.height as usize) * 100
        / app.reader_lines.len().max(1))
    .min(100);
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
