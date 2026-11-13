//! Bounded visual-preview changes are owned with the workspace they edit.

use crate::model::Workspace;
use anyhow::{Result, ensure};

pub(super) fn change_visual(
    workspace: &mut Workspace,
    field: usize,
    forward: bool,
) {
    if let Some(value) = visual_switch(workspace, field) {
        *value = !*value;
        return;
    }
    if let Some((value, minimum, maximum)) = numeric_setting(workspace, field) {
        *value = resize(*value, forward, minimum, maximum);
        return;
    }
    if field == 0 {
        let themes = ["lume", "nord", "gruvbox"];
        let current = themes
            .iter()
            .position(|theme| *theme == workspace.theme)
            .unwrap_or(0);
        let next = (current + if forward { 1 } else { 2 }) % themes.len();
        workspace.theme = themes[next].into();
    }
}

fn resize(value: u16, forward: bool, minimum: u16, maximum: u16) -> u16 {
    if forward {
        value.saturating_add(1).min(maximum)
    } else {
        value.saturating_sub(1).max(minimum)
    }
}

fn visual_switch(workspace: &mut Workspace, field: usize) -> Option<&mut bool> {
    Some(match field {
        4 => &mut workspace.compact,
        5 => &mut workspace.show_library,
        6 => &mut workspace.show_articles,
        8 => &mut workspace.show_reader,
        9 => &mut workspace.show_annotations,
        10 => &mut workspace.mouse,
        12 => &mut workspace.justify,
        13 => &mut workspace.latex_formulas,
        14 => &mut workspace.latex_graphics,
        15 => &mut workspace.auto_read.enabled,
        _ => return None,
    })
}

pub(super) fn apply_value(
    workspace: &mut Workspace,
    field: usize,
    text: &str,
) -> Result<()> {
    if field == 0 {
        ensure!(
            ["lume", "nord", "gruvbox"].contains(&text),
            "Use lume, nord or gruvbox"
        );
        workspace.theme = text.into();
        return Ok(());
    }
    let value: u16 = text
        .parse()
        .map_err(|_| anyhow::anyhow!("Enter a whole number"))?;
    let (destination, minimum, maximum) = numeric_setting(workspace, field)
        .ok_or_else(|| {
            anyhow::anyhow!("This setting has no editable scalar value")
        })?;
    ensure!(
        (minimum..=maximum).contains(&value),
        "Use a value from {minimum} to {maximum}"
    );
    *destination = value;
    Ok(())
}

fn numeric_setting(
    workspace: &mut Workspace,
    field: usize,
) -> Option<(&mut u16, u16, u16)> {
    Some(match field {
        1 => (&mut workspace.reading_width, 40, 140),
        2 => (&mut workspace.library_width, 16, 48),
        3 => (&mut workspace.article_width, 24, 80),
        7 => (&mut workspace.annotation_width, 24, 80),
        11 => (&mut workspace.fullscreen_width, 40, 140),
        16 => (&mut workspace.auto_read.min_active_seconds, 1, 3600),
        17 => (&mut workspace.auto_read.min_coverage_percent, 1, 100),
        _ => return None,
    })
}
