//! Templates insert editable structure without manufacturing study answers.

use anyhow::{Result, ensure};

use super::super::{App, EditMode, Mode};

impl App {
    pub(in crate::app) fn note_editor_command(
        &mut self,
        text: &str,
    ) -> Result<bool> {
        let (verb, argument) = text
            .trim()
            .split_once(char::is_whitespace)
            .unwrap_or((text.trim(), ""));
        match verb {
            "link" => self.note_link_completion(argument.trim())?,
            "template" => self.insert_note_template(argument.trim())?,
            "find" => {
                if let Mode::Input(input) = &mut self.mode {
                    ensure!(argument.len() <= 256, "Search exceeds 256 bytes");
                    input.editor.set_search_pattern(argument)?;
                    input.editor.search_forward(true);
                    input.edit_mode = EditMode::Normal;
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn insert_note_template(&mut self, name: &str) -> Result<()> {
        let (text, kind) = template(name)?;
        let Mode::Input(input) = &mut self.mode else {
            anyhow::bail!("Open a note editor first");
        };
        ensure!(
            input.text.len() + text.len() <= 16384,
            "Draft exceeds 16 KiB"
        );
        ensure!(
            matches!(
                input.kind,
                super::super::InputKind::Comment { .. }
                    | super::super::InputKind::Highlight { .. }
            ),
            "Templates belong to note bodies"
        );
        if !super::super::draft_save::persisted(&input.kind) {
            input.initial_kind = Some(kind);
        }
        input.editor.insert_str(text);
        input.text = input.editor.lines().join("\n");
        input.edit_mode = EditMode::Insert;
        self.observe_draft();
        Ok(())
    }
}

fn template(name: &str) -> Result<(&'static str, crate::model::NoteKind)> {
    let text = match name {
        "study" | "insight" => {
            "## Insight\n\n\n## Evidence\n\n\n## Application\n\n"
        }
        "question" => {
            "## Question\n\n\n## My hypothesis\n\n\n## How to verify\n\n"
        }
        "synthesis" => {
            "# Synthesis\n\n## My conclusion\n\n\n## Evidence\n\n\n## Open questions\n\n"
        }
        "definition" => {
            "## Concept\n\n\n## Definition in my words\n\n\n## Example\n\n"
        }
        "todo" => "## Action\n\n- [ ] \n\n## Success criteria\n\n",
        _ => anyhow::bail!(
            "Use :template study, question, synthesis, definition or todo"
        ),
    };
    let kind = match name {
        "question" => crate::model::NoteKind::Question,
        "definition" => crate::model::NoteKind::Definition,
        "synthesis" => crate::model::NoteKind::Synthesis,
        "todo" => crate::model::NoteKind::Todo,
        _ => crate::model::NoteKind::Insight,
    };
    Ok((text, kind))
}
