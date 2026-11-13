//! Borrowed frame contracts. Each renderer sees only its cached presentation.

use ratatui::layout::Rect;

use super::{
    App, FloatingPane, HelpView, LayoutView, Mode, NavigationView,
    NotebookView, PanelDrag, ReaderView, WebPage,
};
use crate::model::{ArticleTarget, Pane, Workspace};

/// Cached inputs for one frame; construction performs no parsing, I/O or clones.
///
/// UI roots distribute the individual pane contracts to their renderers. No
/// application, storage, worker or mutation capability crosses this boundary.
pub struct PresentationView<'a> {
    pub(crate) layout: LayoutView<'a>,
    pub(crate) navigation: NavigationPaneView<'a>,
    pub(crate) reader: ReaderPaneView<'a>,
    pub(crate) notebook: Option<NotebookView<'a>>,
    pub(crate) chrome: ChromeView<'a>,
    pub(crate) controls: ControlsView<'a>,
    pub(crate) overlays: OverlayView<'a>,
    pub(crate) guide: GuideView<'a>,
    pub(crate) preview: PreviewView<'a>,
    pub(crate) foreground: ForegroundView<'a>,
    pub(crate) web: WebView<'a>,
}

/// Borrowed motion and native-image layers, constructed after pane capture.
pub struct ForegroundView<'a> {
    pub(crate) motion: MotionView<'a>,
    pub(crate) native: NativeImagesView<'a>,
}

#[derive(Clone, Copy)]
pub(crate) struct NavigationPaneView<'a> {
    pub navigation: NavigationView<'a>,
    pub focus: Pane,
    pub compact: bool,
    pub search: SearchView<'a>,
    pub headlines: &'a std::cell::RefCell<super::headlines::Headlines>,
}

#[derive(Clone, Copy)]
pub(crate) struct SearchView<'a> {
    pub mode: &'a Mode,
    pub navigation: NavigationView<'a>,
}

#[derive(Clone, Copy)]
pub(crate) struct ReaderPaneView<'a> {
    pub reader: ReaderView<'a>,
    pub navigation: NavigationView<'a>,
    pub workspace: &'a Workspace,
    pub focus: Pane,
    pub status: &'a str,
    pub stale: bool,
    pub preparing: bool,
    pub fetching: Option<&'a ArticleTarget>,
    pub text: TextView<'a>,
    pub images: super::ImageView<'a>,
}

#[derive(Clone, Copy)]
pub(crate) struct TextView<'a> {
    pub reader: ReaderView<'a>,
    pub focus: Pane,
    pub mode: &'a Mode,
}

#[derive(Clone, Copy)]
pub(crate) struct PreviewView<'a> {
    pub reader: ReaderView<'a>,
    pub mode: &'a Mode,
    pub layout_prefix_pending: bool,
}

pub(crate) struct ChromeView<'a> {
    pub mode: &'a Mode,
    pub focus: Pane,
    pub status: &'a str,
    pub notebook: bool,
    pub demo: bool,
    pub selection: bool,
    pub reader: ReaderView<'a>,
    pub preparing: bool,
    pub reader_search: bool,
    pub layout_prefix_pending: bool,
    pub session_failed: bool,
    pub session_pending: bool,
    pub refreshing: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct ControlsView<'a> {
    pub workspace: &'a Workspace,
    pub focus: Pane,
    pub notebook: bool,
    pub panel_drag: Option<PanelDrag>,
    pub pane_insertion_slot: Option<Rect>,
}

pub(crate) struct OverlayView<'a> {
    pub mode: &'a Mode,
    pub help: HelpView<'a>,
    pub status: &'a str,
    pub workspace: &'a Workspace,
}

#[derive(Clone, Copy)]
pub(crate) struct GuideView<'a> {
    pub mode: &'a Mode,
    pub focus: Pane,
    pub layout_prefix_pending: bool,
}

pub(crate) struct MotionView<'a> {
    pub mode: &'a Mode,
    pub floating_pane: Option<FloatingPane<'a>>,
    pub region: Option<Rect>,
    pub controls: ControlsView<'a>,
    neighbors: Option<&'a super::pane_motion::neighbors::Neighbors>,
}

impl MotionView<'_> {
    pub(crate) fn neighboring_panes(
        &self,
    ) -> impl Iterator<Item = FloatingPane<'_>> {
        self.neighbors
            .into_iter()
            .flat_map(|motion| motion.pictures())
    }
}

pub(crate) struct NativeImagesView<'a> {
    pub images: super::ImageView<'a>,
    pub workspace: &'a Workspace,
    pub focus: Pane,
    pub notebook: bool,
    pub motion_active: bool,
    pub preparing: bool,
    pub stale: bool,
    pub guide: GuideView<'a>,
    pub preview: PreviewView<'a>,
}

pub(crate) struct WebView<'a> {
    pub page: Option<&'a WebPage>,
    pub status: &'a str,
    pub width: u16,
    pub images: super::ImageView<'a>,
}

impl App {
    /// Borrow a coherent set of cached frame inputs without doing frame work.
    pub fn presentation(&self) -> PresentationView<'_> {
        PresentationView {
            layout: self.layout(),
            navigation: self.navigation_pane_view(),
            reader: self.reader_pane_view(),
            notebook: self.notebook(),
            chrome: self.chrome_view(),
            controls: self.controls_view(),
            overlays: OverlayView {
                mode: self.mode(),
                help: self.help(),
                status: self.status(),
                workspace: self.workspace(),
            },
            guide: self.guide_view(),
            preview: self.preview_view(),
            foreground: self.foreground_presentation(),
            web: WebView {
                page: self.web_page(),
                status: self.status(),
                width: self.workspace().reading_width,
                images: self.image_view(),
            },
        }
    }

    /// Borrow just the layers drawn after pane capture, without footer work.
    pub fn foreground_presentation(&self) -> ForegroundView<'_> {
        ForegroundView {
            motion: self.motion_view(),
            native: self.native_images_view(),
        }
    }

    fn navigation_pane_view(&self) -> NavigationPaneView<'_> {
        NavigationPaneView {
            navigation: self.navigation(),
            focus: self.focus(),
            compact: self.workspace().compact,
            search: SearchView {
                mode: self.mode(),
                navigation: self.navigation(),
            },
            headlines: &self.headlines,
        }
    }

    fn reader_pane_view(&self) -> ReaderPaneView<'_> {
        ReaderPaneView {
            reader: self.reader(),
            navigation: self.navigation(),
            workspace: self.workspace(),
            focus: self.focus(),
            status: self.status(),
            stale: self.reader_stale(),
            preparing: self.article_preparing(),
            fetching: self.article_fetching(),
            text: TextView {
                reader: self.reader(),
                focus: self.focus(),
                mode: self.mode(),
            },
            images: self.image_view(),
        }
    }

    fn chrome_view(&self) -> ChromeView<'_> {
        let reader = self.reader();
        ChromeView {
            mode: self.mode(),
            focus: self.focus(),
            status: self.status(),
            notebook: self.notebook().is_some(),
            demo: self.demo(),
            selection: reader.selection.is_some(),
            reader,
            preparing: self.article_preparing(),
            reader_search: !reader.reader_tools.needle.is_empty(),
            layout_prefix_pending: self.layout_prefix_pending(),
            session_failed: self.session_failed(),
            session_pending: self.session_pending(),
            refreshing: self.refreshing(),
        }
    }

    fn controls_view(&self) -> ControlsView<'_> {
        ControlsView {
            workspace: self.workspace(),
            focus: self.focus(),
            notebook: self.notebook().is_some(),
            panel_drag: self.panel_drag(),
            pane_insertion_slot: self.pane_insertion_slot(),
        }
    }

    fn guide_view(&self) -> GuideView<'_> {
        GuideView {
            mode: self.mode(),
            focus: self.focus(),
            layout_prefix_pending: self.layout_prefix_pending(),
        }
    }

    fn preview_view(&self) -> PreviewView<'_> {
        PreviewView {
            reader: self.reader(),
            mode: self.mode(),
            layout_prefix_pending: self.layout_prefix_pending(),
        }
    }

    fn motion_view(&self) -> MotionView<'_> {
        MotionView {
            mode: self.mode(),
            floating_pane: self.floating_pane(),
            region: self.pane_motion_region(),
            controls: self.controls_view(),
            neighbors: self.neighbors(),
        }
    }

    fn native_images_view(&self) -> NativeImagesView<'_> {
        NativeImagesView {
            images: self.image_view(),
            workspace: self.workspace(),
            focus: self.focus(),
            notebook: self.notebook().is_some(),
            motion_active: self.pane_motion_active(),
            preparing: self.article_preparing(),
            stale: self.reader_stale(),
            guide: self.guide_view(),
            preview: self.preview_view(),
        }
    }
}

#[cfg(test)]
mod tests;
