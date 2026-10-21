//! Public Markdown links, native preview ordering and executable TOML examples.

use std::{fs, path::Path};

use anyhow::{Result, ensure};
use pulldown_cmark::{Event, Parser, Tag};

#[test]
fn public_guides_link_to_existing_local_documents_and_assets() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = ["README.md", "AGENTS.md"]
        .map(|name| root.join(name))
        .to_vec();
    for file in fs::read_dir(root.join("docs"))? {
        let path = file?.path();
        if path.extension().is_some_and(|extension| extension == "md") {
            paths.push(path);
        }
    }
    for path in paths {
        let text = fs::read_to_string(&path)?;
        for event in Parser::new(&text) {
            if let Event::Start(
                Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. },
            ) = event
            {
                check_link(&path, &dest_url)?;
            }
        }
    }
    Ok(())
}

fn check_link(document: &Path, destination: &str) -> Result<()> {
    if destination.contains("://") || destination.starts_with('#') {
        return Ok(());
    }
    let file = destination.split('#').next().unwrap_or_default();
    let parent = document.parent().ok_or_else(|| {
        anyhow::anyhow!("Document has no parent: {}", document.display())
    })?;
    ensure!(
        parent.join(file).exists(),
        "Broken local link in {}: {destination}",
        document.display()
    );
    Ok(())
}

#[test]
fn first_readme_image_is_the_native_lighthouse_welcome() -> Result<()> {
    let text = include_str!("../README.md");
    let first = Parser::new(text).find_map(|event| match event {
        Event::Start(Tag::Image { dest_url, .. }) => {
            Some(dest_url.into_string())
        }
        _ => None,
    });
    assert_eq!(first.as_deref(), Some("docs/preview-welcome.png"));
    Ok(())
}

#[test]
fn documented_toml_examples_decode_as_one_portable_configuration() -> Result<()>
{
    let text = include_str!("../docs/configuration.md");
    let mut combined = String::new();
    let mut inside = false;
    for line in text.lines() {
        if line == "```toml" {
            inside = true;
        } else if line == "```" {
            inside = false;
        } else if inside && !line.starts_with("schema_version") {
            combined.push_str(line);
            combined.push('\n');
        }
    }
    let config = lume::config::decode(&combined)?;
    assert_eq!(config.feeds[0].id, "journal");
    assert_eq!(config.ui.layout.fullscreen_width, 80);
    let example =
        lume::config::decode(include_str!("../examples/config.toml"))?;
    assert_eq!(
        example.note_collections[0].query,
        "type:question status:open"
    );
    Ok(())
}
