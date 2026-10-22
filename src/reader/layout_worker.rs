//! Markdown parsing and wrapping run independently of syntax and input.

use std::sync::Arc;

use anyhow::Result;
use ratatui::text::Line;

use crate::latest::Latest;

use super::{RenderOptions, Rendered, live::CodeKey};

type Styles = Vec<(CodeKey, Arc<[Line<'static>]>)>;

struct Request {
    body: Arc<str>,
    width: usize,
    styles: Styles,
    options: RenderOptions,
}

struct Completed {
    body: Arc<str>,
    width: usize,
    rendered: Rendered,
    blocks: Vec<CodeKey>,
    options: RenderOptions,
}

pub(super) struct LayoutWorker {
    worker: Latest<Request, Completed>,
    requested: Option<(Arc<str>, usize, u64)>,
    ready: Option<Completed>,
    revision: u64,
    options: RenderOptions,
    pub(super) error: Option<String>,
}

impl LayoutWorker {
    pub(super) fn start(options: RenderOptions) -> Result<Self> {
        Ok(Self {
            worker: Latest::start("lume-layout", prepare)?,
            requested: None,
            ready: None,
            revision: 0,
            options,
            error: None,
        })
    }

    pub(super) fn set_options(&mut self, options: RenderOptions) {
        self.options = options;
        self.worker.cancel();
        self.requested = None;
        self.revision = self.revision.wrapping_add(1);
    }

    pub(super) fn render(
        &mut self,
        body: &str,
        width: usize,
        styles: impl FnOnce() -> Styles,
    ) -> Rendered {
        let same = self.requested(body, width);
        if !same {
            let body: Arc<str> = Arc::from(body);
            self.requested = Some((body.clone(), width, self.revision));
            self.worker.submit(Request {
                body,
                width,
                styles: styles(),
                options: self.options,
            });
        }
        self.ready
            .as_ref()
            .filter(|ready| ready.body.as_ref() == body)
            .map(|ready| ready.rendered.clone())
            .unwrap_or_default()
    }

    pub(super) fn requested(&self, body: &str, width: usize) -> bool {
        self.requested
            .as_ref()
            .is_some_and(|(source, columns, revision)| {
                source.as_ref() == body
                    && *columns == width
                    && *revision == self.revision
            })
    }

    pub(super) fn poll(&mut self) -> Option<Vec<CodeKey>> {
        let completed = self.worker.poll()?;
        match completed {
            Ok(completed) => {
                let blocks = completed.blocks.clone();
                self.ready = Some(completed);
                Some(blocks)
            }
            Err(error) => {
                self.error = Some(error.to_string());
                Some(Vec::new())
            }
        }
    }

    pub(super) fn seed(
        &mut self,
        body: &str,
        width: usize,
        rendered: &Rendered,
    ) {
        self.worker.cancel();
        let body: Arc<str> = Arc::from(body);
        self.requested = Some((body.clone(), width, self.revision));
        self.ready = Some(Completed {
            body,
            width,
            rendered: rendered.clone(),
            blocks: Vec::new(),
            options: self.options,
        });
    }

    pub(super) fn invalidate_styles(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub(super) fn source_ready(&self, body: &str) -> bool {
        self.ready.as_ref().is_some_and(|ready| {
            ready.body.as_ref() == body && ready.options == self.options
        })
    }

    pub(super) fn preparing(&self, body: &str, width: usize) -> bool {
        !self.ready.as_ref().is_some_and(|ready| {
            ready.body.as_ref() == body
                && ready.width == width
                && ready.options == self.options
        })
    }

    pub(super) fn pending(&self) -> bool {
        self.worker.pending()
    }
}

fn prepare(request: Request) -> Result<Completed> {
    let mut blocks = Vec::new();
    let rendered = super::render_with(
        &request.body,
        request.width,
        request.options,
        &mut |text, language| {
            let key = CodeKey {
                text: Arc::from(text),
                language: language.into(),
            };
            let lines = request
                .styles
                .iter()
                .find(|(cached, _)| *cached == key)
                .map(|(_, lines)| lines.to_vec())
                .unwrap_or_else(|| {
                    text.split_inclusive('\n')
                        .map(|line| {
                            Line::raw(line.trim_end_matches('\n').to_owned())
                        })
                        .collect()
                });
            blocks.push(key);
            lines
        },
    );
    Ok(Completed {
        body: request.body,
        width: request.width,
        rendered,
        blocks,
        options: request.options,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn unchanged_requests_do_not_rebuild_syntax_snapshots() -> Result<()> {
        let mut worker = LayoutWorker::start(RenderOptions::default())?;
        let snapshots = Cell::new(0);
        let styles = || {
            snapshots.set(snapshots.get() + 1);
            Vec::new()
        };

        worker.render("First source", 60, styles);
        worker.render("First source", 60, styles);
        assert_eq!(snapshots.get(), 1);
        worker.render("First source", 80, styles);
        assert_eq!(snapshots.get(), 2);
        worker.invalidate_styles();
        worker.render("First source", 80, styles);
        assert_eq!(snapshots.get(), 3);
        worker.render("Second source", 80, styles);
        assert_eq!(snapshots.get(), 4);

        let rendered = super::super::render("Second source", 80);
        worker.seed("Second source", 80, &rendered);
        assert_eq!(
            worker.render("Second source", 80, styles).text,
            rendered.text
        );
        assert_eq!(snapshots.get(), 4);
        Ok(())
    }

    #[test]
    fn admitted_styles_survive_eviction_and_preserve_code_bytes() -> Result<()>
    {
        let text = "let café = 42;\n";
        let key = CodeKey {
            text: Arc::from(text),
            language: "rust".into(),
        };
        let colored = Line::styled(
            text.trim_end_matches('\n').to_owned(),
            ratatui::style::Style::new().fg(ratatui::style::Color::Red),
        );
        let cache: Arc<[Line<'static>]> = vec![colored.clone()].into();
        let styles = vec![(key, cache.clone())];
        drop(cache);

        let completed = prepare(Request {
            body: Arc::from(format!("```rust\n{text}```")),
            width: 60,
            styles,
            options: RenderOptions::default(),
        })?;

        assert!(completed.rendered.lines.contains(&colored));
        assert!(completed.rendered.text.contains(text.trim_end()));
        assert_eq!(completed.blocks.len(), 1);
        Ok(())
    }
}
