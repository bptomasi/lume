//! Inspect actual Crossterm output across cached native-view transitions.
use super::*;
use ratatui::{
    TerminalOptions, Viewport, backend::CrosstermBackend, layout::Rect,
};
use std::{cell::RefCell, io::Write, rc::Rc};

const PIXELS: &str = "\x1bPq\"1;1;20;20#0;2;100;100;100!20~\x1b\\";

#[derive(Default)]
struct Output {
    bytes: Vec<u8>,
    fail_clear: bool,
    fail_pixels: bool,
}

#[derive(Clone, Default)]
struct Capture(Rc<RefCell<Output>>);

impl Capture {
    fn take(&self) -> io::Result<Vec<u8>> {
        let mut output = self.0.try_borrow_mut().map_err(io::Error::other)?;
        Ok(std::mem::take(&mut output.bytes))
    }
}

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut output = self.0.try_borrow_mut().map_err(io::Error::other)?;
        if std::mem::take(&mut output.fail_clear)
            || (output.fail_pixels && bytes == PIXELS.as_bytes())
        {
            output.fail_pixels = false;
            return Err(io::Error::other("Injected presentation failure"));
        }
        output.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn terminal(
    capture: Capture,
) -> io::Result<Terminal<CrosstermBackend<Capture>>> {
    Terminal::with_options(
        CrosstermBackend::new(capture),
        TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 20, 8)),
        },
    )
}

fn paint(frame: &mut Frame<'_>, inspector: bool) {
    frame.buffer_mut().reset();
    if inspector {
        frame.buffer_mut()[(2, 2)].set_symbol(PIXELS);
        frame.buffer_mut()[(3, 2)].set_skip(true);
    } else {
        frame.buffer_mut()[(2, 2)].set_symbol("Reader");
        frame.buffer_mut()[(2, 6)].set_symbol("Text below the image");
    }
}

fn cleared(bytes: &[u8]) -> bool {
    bytes.windows(3).any(|part| part == b"\x1b[J")
}

#[test]
fn reopening_cached_pixels_after_reader_scroll_clears_before_every_transition()
-> io::Result<()> {
    let capture = Capture::default();
    let mut terminal = terminal(capture.clone())?;
    let mut presentation = Presentation::default();
    for _ in 0..3 {
        presentation.draw(&mut terminal, false, |frame| paint(frame, false))?;
        capture.take()?;
        presentation.draw(&mut terminal, true, |frame| paint(frame, true))?;
        let bytes = capture.take()?;
        assert!(cleared(&bytes));
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.find("\x1b[J") < text.find(PIXELS));
        assert!(!text.contains("Text below the image"));
        presentation.draw(&mut terminal, true, |frame| paint(frame, true))?;
        assert!(!cleared(&capture.take()?));
        presentation.draw(&mut terminal, false, |frame| paint(frame, false))?;
        let bytes = capture.take()?;
        assert!(cleared(&bytes));
        assert!(
            String::from_utf8_lossy(&bytes).contains("Text below the image")
        );
    }
    Ok(())
}

#[test]
fn failed_clear_or_image_output_retries_the_full_transition() -> io::Result<()>
{
    for fail_pixels in [false, true] {
        let capture = Capture::default();
        let mut terminal = terminal(capture.clone())?;
        let mut presentation = Presentation::default();
        {
            let mut output =
                capture.0.try_borrow_mut().map_err(io::Error::other)?;
            output.fail_clear = !fail_pixels;
            output.fail_pixels = fail_pixels;
        }
        assert!(
            presentation
                .draw(&mut terminal, true, |frame| paint(frame, true))
                .is_err()
        );
        capture.take()?;
        presentation.draw(&mut terminal, true, |frame| paint(frame, true))?;
        assert!(cleared(&capture.take()?));
    }
    Ok(())
}
