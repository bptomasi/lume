//! Example-local export format shared by static and animated buffer previews.

use std::{fs, path::Path};

use anyhow::Result;
use ratatui::{buffer::Buffer, style::Color};

pub fn export(buffer: &Buffer, output: &Path) -> Result<()> {
    let cells = buffer
        .content
        .iter()
        .map(|cell| (cell.symbol(), rgb(cell.fg), rgb(cell.bg)))
        .collect::<Vec<_>>();
    let snapshot = serde_json::json!({
        "width": buffer.area.width,
        "height": buffer.area.height,
        "cells": cells,
    });
    fs::write(output, serde_json::to_vec(&snapshot)?)?;
    Ok(())
}

fn rgb(color: Color) -> String {
    match color {
        Color::Rgb(red, green, blue) => {
            format!("#{red:02x}{green:02x}{blue:02x}")
        }
        _ => "#d6deeb".into(),
    }
}
