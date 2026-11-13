//! Modal ownership and transient keyboard intent, separate from adapters.
//!
//! Editors are borrowed by their concrete type. A caller cannot replace the
//! active mode through a mutable reference, or accidentally edit a Copy view.

use super::{
    DocumentView, ImageInspector, Input, Mode, Picker, ServerToken,
    SettingsDraft, SmartForm, View,
};

pub(super) enum ModalScroll {
    Unhandled,
    Handled,
    Picker,
}

pub(super) struct InteractionState {
    mode: Mode,
    status: String,
    pending_g: bool,
    panel_prefix: bool,
    destination_delete: Option<View>,
    article_delete: Option<i64>,
    help: super::help::HelpState,
    vim: super::vim::Vim,
}

impl Default for InteractionState {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            status:
                "Welcome to Lume · ? for shortcuts · :feed URL to subscribe"
                    .into(),
            pending_g: false,
            panel_prefix: false,
            destination_delete: None,
            article_delete: None,
            help: super::help::HelpState::default(),
            vim: super::vim::Vim::default(),
        }
    }
}

impl InteractionState {
    pub(super) fn help(&self) -> &super::help::HelpState {
        &self.help
    }

    pub(super) fn help_input(&mut self) -> &mut super::help::HelpState {
        &mut self.help
    }

    pub(super) fn vim(&self) -> &super::vim::Vim {
        &self.vim
    }

    pub(super) fn vim_input(&mut self) -> &mut super::vim::Vim {
        &mut self.vim
    }

    pub(super) fn mode(&self) -> &Mode {
        &self.mode
    }

    /// Modal entry consumes navigation prefixes and captured confirmations.
    /// Restoring a captured draft changes only the mode: its editor/history,
    /// original target, and saved baseline move together in the boxed Input.
    pub(super) fn enter(&mut self, mode: Mode) {
        if !matches!(mode, Mode::Normal) {
            self.pending_g = false;
            self.panel_prefix = false;
            self.vim.clear_input();
            self.cancel_deletions();
        }
        self.mode = mode;
    }

    pub(super) fn begin_input(
        &mut self,
        kind: super::InputKind,
        text: String,
        previous_search: String,
    ) {
        let mut input = Input::new(kind, text, previous_search);
        if matches!(input.kind, super::InputKind::NoteSearch) {
            input.previous_search.clone_from(&input.text);
        }
        self.enter(Mode::Input(Box::new(input)));
    }

    /// Ordinary keys cancel confirmation; d/Escape preserve it until the
    /// targeted operator consumes it, including repeated key events.
    pub(super) fn observe_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};
        let cancel = !matches!(key.code, KeyCode::Char('d') | KeyCode::Esc)
            || key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        if cancel {
            self.cancel_deletions();
        }
        cancel
    }

    /// Scroll modal-owned content; external page/image actors stay coordinated.
    pub(super) fn scroll_overlay(
        &mut self,
        forward: bool,
        amount: usize,
    ) -> ModalScroll {
        match &mut self.mode {
            Mode::Help => self.help.scroll(forward, amount),
            Mode::Document(document) => {
                document.row = shifted(
                    document.row,
                    document.lines.len(),
                    forward,
                    amount,
                );
            }
            Mode::Picker(picker) => {
                picker.selected = shifted(
                    picker.selected,
                    picker.choices.len(),
                    forward,
                    amount,
                );
                return ModalScroll::Picker;
            }
            Mode::Settings(draft) if draft.editor.is_none() => {
                draft.field = shifted(
                    draft.field,
                    SettingsDraft::FIELD_COUNT,
                    forward,
                    amount,
                );
            }
            _ => return ModalScroll::Unhandled,
        }
        ModalScroll::Handled
    }

    /// Move a captured modal out while its submit operation runs.
    pub(super) fn take_mode(&mut self) -> Mode {
        std::mem::take(&mut self.mode)
    }

    pub(super) fn status(&self) -> &str {
        &self.status
    }

    pub(super) fn report(&mut self, status: String) {
        self.status = status;
    }

    pub(super) fn draft_mut(&mut self) -> Option<&mut Input> {
        match &mut self.mode {
            Mode::Input(input) => Some(input),
            _ => None,
        }
    }

    pub(super) fn picker_mut(&mut self) -> Option<&mut Picker> {
        match &mut self.mode {
            Mode::Picker(picker) => Some(picker),
            _ => None,
        }
    }

    pub(super) fn settings_mut(&mut self) -> Option<&mut SettingsDraft> {
        match &mut self.mode {
            Mode::Settings(settings) => Some(settings),
            _ => None,
        }
    }

    pub(super) fn document_mut(&mut self) -> Option<&mut DocumentView> {
        match &mut self.mode {
            Mode::Document(document) => Some(document),
            _ => None,
        }
    }

    pub(super) fn image_mut(&mut self) -> Option<&mut ImageInspector> {
        match &mut self.mode {
            Mode::Image(image) => Some(image),
            _ => None,
        }
    }

    pub(super) fn smart_mut(&mut self) -> Option<&mut SmartForm> {
        match &mut self.mode {
            Mode::Smart(form) => Some(form),
            _ => None,
        }
    }

    pub(super) fn token_mut(&mut self) -> Option<&mut ServerToken> {
        match &mut self.mode {
            Mode::ServerToken(token) => Some(token),
            _ => None,
        }
    }

    pub(super) fn take_g_prefix(&mut self) -> bool {
        std::mem::take(&mut self.pending_g)
    }

    pub(super) fn set_g_prefix(&mut self, pending: bool) {
        self.pending_g = pending;
    }

    pub(super) fn panel_prefix(&self) -> bool {
        self.panel_prefix
    }

    pub(super) fn set_panel_prefix(&mut self, pending: bool) {
        self.panel_prefix = pending;
    }

    pub(super) fn destination_delete(&self) -> Option<&View> {
        self.destination_delete.as_ref()
    }

    pub(super) fn arm_destination_delete(&mut self, target: Option<View>) {
        self.destination_delete = target;
    }

    pub(super) fn article_delete(&self) -> Option<i64> {
        self.article_delete
    }

    pub(super) fn arm_article_delete(&mut self, target: Option<i64>) {
        self.article_delete = target;
    }

    pub(super) fn take_destination_delete(&mut self) -> Option<View> {
        self.destination_delete.take()
    }

    pub(super) fn take_article_delete(&mut self) -> Option<i64> {
        self.article_delete.take()
    }

    pub(super) fn cancel_deletions(&mut self) {
        self.destination_delete = None;
        self.article_delete = None;
    }
}

fn shifted(
    current: usize,
    length: usize,
    forward: bool,
    amount: usize,
) -> usize {
    if forward {
        current.saturating_add(amount).min(length.saturating_sub(1))
    } else {
        current.saturating_sub(amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context, Result};
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn modal_entry_cancels_navigation_prefixes_and_captured_confirmations() {
        let mut state = InteractionState::default();
        state.set_g_prefix(true);
        state.set_panel_prefix(true);
        state.vim_input().push_digit(4);
        state.arm_destination_delete(Some(View::Feed(9)));
        state.arm_article_delete(Some(17));

        state.enter(Mode::Help);

        assert!(!state.take_g_prefix());
        assert!(!state.panel_prefix());
        assert_eq!(state.vim().count(), 0);
        assert!(state.destination_delete().is_none());
        assert!(state.article_delete().is_none());
    }

    #[test]
    fn confirmation_survives_its_operator_and_escape_but_other_input_cancels() {
        let mut state = InteractionState::default();
        state.arm_article_delete(Some(17));
        assert!(!state.observe_key(KeyCode::Char('d').into()));
        assert!(!state.observe_key(KeyCode::Esc.into()));
        assert_eq!(state.article_delete(), Some(17));
        assert!(state.observe_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('d'),
            KeyModifiers::CONTROL,
        )));
        assert!(state.article_delete().is_none());
        state.arm_destination_delete(Some(View::Feed(9)));
        assert!(state.observe_key(KeyCode::Char('j').into()));
        assert!(state.destination_delete().is_none());
    }

    #[test]
    fn captured_dirty_draft_round_trip_retains_target_baseline_and_editor()
    -> Result<()> {
        let mut state = InteractionState::default();
        state.begin_input(
            super::super::InputKind::Comment {
                article: 27,
                comment: Some(5),
            },
            "Saved baseline".into(),
            "old search".into(),
        );
        let draft = state.draft_mut().context("Missing editor")?;
        draft.editor.insert_str(" new revision");
        draft.text = draft.editor.lines().join("\n");
        draft.edit_mode = super::super::EditMode::Command("wqa".into());
        let text = draft.text.clone();
        let cursor = draft.editor.cursor();
        let captured = state.take_mode();
        assert!(matches!(state.mode(), Mode::Normal));

        // A coordinator may fail while saving; restoring the owned mode must
        // retain the entire draft, including its captured identity and command.
        state.enter(captured);
        let draft = state.draft_mut().context("Lost editor after failure")?;
        assert!(matches!(
            draft.kind,
            super::super::InputKind::Comment {
                article: 27,
                comment: Some(5),
            }
        ));
        assert_eq!(draft.saved_text, "Saved baseline");
        assert_eq!(draft.text, text);
        assert_eq!(draft.editor.cursor(), cursor);
        assert_eq!(
            draft.edit_mode,
            super::super::EditMode::Command("wqa".into())
        );
        assert!(draft.modified());
        Ok(())
    }
}
