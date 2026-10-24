//! Inspecting one protected article must not materialize every smart-list member.
use anyhow::{Result, ensure};
use lume::model::SmartFilter;

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let count = if stress { 10_000 } else { 1000 };
    let store = super::retention::prepare(directory.path(), count)?;
    for index in 0..32 {
        store.save_smart_list(
            &format!("Broad query {index}"),
            &SmartFilter {
                text: format!("NOT neverreviewtoken{index}"),
                ..Default::default()
            },
        )?;
    }
    super::measure(
        &format!(
            "one retention explanation ({count} articles, 32 broad queries)"
        ),
        iterations.min(8),
        || {
            let text =
                store.explain_retention(count as i64 - 1, 1_800_000_000)?;
            ensure!(
                text.contains("matches a protected smart list"),
                "Smart protection missing"
            );
            ensure!(
                text.contains(&format!("Article {}:", count - 1)),
                "Wrong article explained"
            );
            Ok(())
        },
    )
}
