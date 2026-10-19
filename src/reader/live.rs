//! Interactive layout never runs a syntax grammar on the input thread.
//!
//! One worker and one-slot request/result queues bound outstanding work.
//! Exact code/language keys prevent stale colors after navigation or refresh.
//! Results enter a 4 MiB approximate LRU; oversized/evicted blocks stay plain
//! for the current document, avoiding an endless cache eviction/requeue cycle.
//! Drop disconnects both queues; an in-flight grammar may finish independently.

use std::{
    collections::{HashSet, VecDeque},
    mem::size_of,
    sync::{Arc, mpsc},
    thread,
};

use anyhow::Result;
use ratatui::text::{Line, Span};

use super::Rendered;

const CACHE_BUDGET: usize = 4 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct CodeKey {
    pub(super) text: Arc<str>,
    pub(super) language: String,
}

struct Completed {
    key: CodeKey,
    lines: Vec<Line<'static>>,
}

struct Cached {
    completed: Completed,
    bytes: usize,
}

struct ImmediateLayout {
    body: Arc<str>,
    width: usize,
    rendered: Rendered,
}

/// A bounded syntax worker with immediate plain-code layout and exact caching.
pub struct LiveRenderer {
    requests: mpsc::SyncSender<CodeKey>,
    results: mpsc::Receiver<Completed>,
    cache: VecDeque<Cached>,
    retained_bytes: usize,
    submitted: HashSet<CodeKey>,
    pending: HashSet<CodeKey>,
    document: String,
    stopped: bool,
    layout: Option<super::layout_worker::LayoutWorker>,
    immediate: Option<ImmediateLayout>,
}

impl LiveRenderer {
    /// Start one syntax worker; no grammar work occurs during construction.
    ///
    /// # Errors
    /// Returns an operating-system thread creation failure.
    pub fn start() -> Result<Self> {
        Self::spawn(super::syntax::highlight)
    }

    fn spawn(
        highlight: impl Fn(&str, &str) -> Vec<Line<'static>> + Send + 'static,
    ) -> Result<Self> {
        let (requests, receiver) = mpsc::sync_channel::<CodeKey>(1);
        let (sender, results) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("lume-syntax".into())
            .spawn(move || {
                while let Ok(key) = receiver.recv() {
                    let lines = highlight(&key.text, &key.language);
                    if sender.send(Completed { key, lines }).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests,
            results,
            cache: VecDeque::new(),
            retained_bytes: 0,
            submitted: HashSet::new(),
            pending: HashSet::new(),
            document: String::new(),
            stopped: false,
            layout: None,
            immediate: None,
        })
    }

    /// Move Markdown parsing/wrapping onto a separate bounded worker.
    ///
    /// # Errors
    /// Returns thread creation failures without changing synchronous operation.
    pub fn enable_background_layout(&mut self) -> Result<()> {
        let mut layout = super::layout_worker::LayoutWorker::start()?;
        if let Some(cached) = &self.immediate {
            layout.seed(&cached.body, cached.width, &cached.rendered);
        }
        self.layout = Some(layout);
        Ok(())
    }

    /// Prepare Markdown immediately, using cached colors or readable plain code.
    pub fn render(&mut self, body: &str, width: usize) -> Rendered {
        if self.document != body {
            self.document = body.to_owned();
            self.submitted.clear();
        }
        if self
            .layout
            .as_ref()
            .is_some_and(|layout| layout.error.is_some())
        {
            self.layout = None;
        }
        if let Some(layout) = &mut self.layout {
            let styles = self
                .cache
                .iter()
                .map(|cached| {
                    (
                        cached.completed.key.clone(),
                        cached.completed.lines.clone(),
                    )
                })
                .collect();
            return layout.render(body, width, styles);
        }
        self.render_now(body, width)
    }

    /// Prepare an explicit source-bound jump immediately using cached/plain code.
    pub(crate) fn render_now(&mut self, body: &str, width: usize) -> Rendered {
        if let Some(cached) = &self.immediate
            && cached.body.as_ref() == body
            && cached.width == width
        {
            return cached.rendered.clone();
        }
        if self.document != body {
            self.document = body.to_owned();
            self.submitted.clear();
        }
        let rendered =
            super::render_with(body, width, &mut |text, language| {
                self.code(text, language)
            });
        if let Some(layout) = &mut self.layout {
            layout.seed(body, width, &rendered);
        }
        self.immediate = Some(ImmediateLayout {
            body: Arc::from(body),
            width,
            rendered: rendered.clone(),
        });
        rendered
    }

    /// Collect completed syntax work without waiting or accessing storage.
    pub fn poll(&mut self) -> bool {
        let blocks = self.layout.as_mut().and_then(|layout| layout.poll());
        let mut changed = blocks.is_some();
        for key in blocks.into_iter().flatten() {
            self.code(&key.text, &key.language);
        }
        loop {
            match self.results.try_recv() {
                Ok(completed) => {
                    self.immediate = None;
                    self.pending.remove(&completed.key);
                    self.retain(completed);
                    if let Some(layout) = &mut self.layout {
                        layout.invalidate_styles();
                    }
                    changed = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    changed |= !self.stopped;
                    self.stopped = true;
                    self.pending.clear();
                    break;
                }
            }
        }
        changed
    }

    pub(crate) fn source_ready(&self, body: &str) -> bool {
        self.layout
            .as_ref()
            .is_none_or(|layout| layout.source_ready(body))
    }

    pub(crate) fn preparing(&self, body: &str, width: usize) -> bool {
        self.layout
            .as_ref()
            .is_some_and(|layout| layout.preparing(body, width))
    }

    /// Whether admitted jobs have not yet delivered their result.
    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
            || self.layout.as_ref().is_some_and(|layout| layout.pending())
    }

    /// Whether the worker disconnected; readable plain code remains available.
    pub fn is_stopped(&self) -> bool {
        self.stopped
            || self
                .layout
                .as_ref()
                .is_some_and(|layout| layout.error.is_some())
    }

    fn code(&mut self, text: &str, language: &str) -> Vec<Line<'static>> {
        if let Some(index) = self.cache.iter().position(|entry| {
            entry.completed.key.text.as_ref() == text
                && entry.completed.key.language == language
        }) && let Some(entry) = self.cache.remove(index)
        {
            let lines = entry.completed.lines.clone();
            self.cache.push_back(entry);
            return lines;
        }
        if !self.stopped
            && text.len() <= 65_536
            && language.len() <= 256
            && !language.trim().is_empty()
            && !text.lines().any(|line| line.len() > 4096)
        {
            let key = CodeKey {
                text: Arc::from(text),
                language: language.to_owned(),
            };
            self.admit(key);
        }
        text.split_inclusive('\n')
            .map(|line| Line::raw(line.trim_end_matches('\n').to_owned()))
            .collect()
    }

    fn admit(&mut self, key: CodeKey) {
        if self.pending.contains(&key) {
            self.submitted.insert(key);
            return;
        }
        if self.submitted.contains(&key) {
            return;
        }
        match self.requests.try_send(key.clone()) {
            Ok(()) => {
                self.pending.insert(key.clone());
                self.submitted.insert(key);
            }
            Err(mpsc::TrySendError::Full(_)) => {}
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.stopped = true;
                self.pending.clear();
            }
        }
    }

    fn retain(&mut self, completed: Completed) {
        let bytes = retained_size(&completed);
        if bytes > CACHE_BUDGET {
            return;
        }
        while self.retained_bytes + bytes > CACHE_BUDGET {
            if let Some(oldest) = self.cache.pop_front() {
                self.retained_bytes -= oldest.bytes;
            }
        }
        self.retained_bytes += bytes;
        self.cache.push_back(Cached { completed, bytes });
    }
}

fn retained_size(completed: &Completed) -> usize {
    completed.key.text.len()
        + completed.key.language.len()
        + completed.lines.capacity() * size_of::<Line<'static>>()
        + completed
            .lines
            .iter()
            .map(|line| {
                line.spans.capacity() * size_of::<Span<'static>>()
                    + line
                        .spans
                        .iter()
                        .map(|span| span.content.len())
                        .sum::<usize>()
            })
            .sum::<usize>()
}

#[cfg(test)]
mod tests;
