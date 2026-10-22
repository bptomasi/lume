//! Responsive geometry covers steady frames and changing split dimensions.

use std::hint::black_box;

use anyhow::{Result, ensure};
use lume::{
    model::{Dock, Pane, PaneLayout, Workspace},
    ui,
};
use ratatui::layout::Rect;

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let count = if stress { 400 } else { 40 };
    let classic = Workspace::default();
    let split = PaneLayout::columns(&Pane::ALL)
        .dock(Pane::Annotations, Pane::Reader, Dock::Below)
        .ok_or_else(|| anyhow::anyhow!("Invalid geometry fixture"))?;
    let custom = Workspace {
        layout: Some(split),
        show_annotations: true,
        ..Workspace::default()
    };
    println!("geometry input · {count} widths · classic/custom · four focuses");
    super::measure("responsive pane geometry", iterations * 2, || {
        for index in 0..count {
            let screen = Rect::new(7, 3, 40 + index * 4, 44);
            for workspace in [&classic, &custom] {
                for focus in Pane::ALL {
                    let areas = black_box(ui::workspace_areas(
                        screen, focus, workspace,
                    ));
                    let mut cells = 0;
                    for pane in Pane::ALL {
                        if let Some(area) = areas.get(pane) {
                            ensure!(
                                screen.intersection(area) == area,
                                "Pane escaped the terminal"
                            );
                            cells +=
                                u32::from(area.width) * u32::from(area.height);
                        }
                    }
                    ensure!(
                        cells == u32::from(screen.width) * 40,
                        "Responsive geometry left gaps or overlaps"
                    );
                }
            }
        }
        Ok(())
    })
}
