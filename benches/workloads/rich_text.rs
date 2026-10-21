//! Dense paragraphs check every link, complete text and a final source anchor.

use anyhow::{Result, ensure};
use lume::reader;

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let count = if stress { 2000 } else { 200 };
    let (source, expected) = fixture(count);
    println!(
        "rich paragraph input · links={count} · bytes={}",
        source.len()
    );
    let _ = reader::render("Warmup.", 80);
    super::measure("dense paragraph layout + source checks", iterations, || {
        let rendered = reader::render(&source, 80);
        ensure!(rendered.text == expected, "Readable paragraph changed");
        ensure!(rendered.links.len() == count, "Link identities were lost");
        for (index, (title, url)) in rendered.links.iter().enumerate() {
            ensure!(title == &format!("ref-{index}"), "Link label drifted");
            ensure!(
                url == &format!("https://journal.example/ref/{index}"),
                "Link order drifted"
            );
        }
        let last = format!("ref-{}", count - 1);
        let start = rendered
            .text
            .rfind(&last)
            .ok_or_else(|| anyhow::anyhow!("Final link missing"))?;
        let anchor =
            reader::to_source(&rendered.mappings, start..start + last.len())
                .ok_or_else(|| anyhow::anyhow!("Link source lost"))?;
        ensure!(source.get(anchor) == Some(last.as_str()), "Source drifted");
        Ok(())
    })
}

fn fixture(count: usize) -> (String, String) {
    let mut source = String::new();
    let mut expected = String::new();
    for index in 0..count {
        source.push_str("alpha **beta** `gamma` &amp; 界 ");
        source.push_str(&format!(
            "[ref-{index}](https://journal.example/ref/{index}) "
        ));
        expected.push_str(&format!("alpha beta gamma & 界 ref-{index} "));
    }
    source.push_str("Done.");
    expected.push_str("Done.\n\n");
    (source, expected)
}
