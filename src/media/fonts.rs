//! One bounded fallback font replaces system-wide SVG font discovery.

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};
use resvg::usvg;

const MAX_FONT_BYTES: usize = 4 * 1024 * 1024;

pub(super) fn prepare(options: &mut usvg::Options<'_>) -> Result<()> {
    if options.fontdb.faces().next().is_some() {
        return Ok(());
    }
    for path in candidates() {
        if load_font(options, &path)? {
            return Ok(());
        }
    }
    bail!(
        "SVG text needs a fallback font; set LUME_SVG_FONT to a TTF/OTF file within 4 MiB"
    )
}

fn load_font(options: &mut usvg::Options<'_>, path: &Path) -> Result<bool> {
    let Ok(file) = File::open(path) else {
        return Ok(false);
    };
    let mut bytes = Vec::new();
    file.take((MAX_FONT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_FONT_BYTES {
        return Ok(false);
    }
    options.fontdb_mut().load_font_data(bytes);
    let family = options
        .fontdb
        .faces()
        .next()
        .and_then(|face| face.families.first())
        .map(|(name, _)| name.clone());
    let Some(family) = family else {
        return Ok(false);
    };
    options.fontdb_mut().set_sans_serif_family(&family);
    options.font_family = family;
    Ok(true)
}

fn candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = std::env::var_os("LUME_SVG_FONT") {
        paths.push(PathBuf::from(path));
    }
    paths.extend([
        PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
        PathBuf::from("/usr/share/fonts/TTF/DejaVuSans.ttf"),
        PathBuf::from(
            "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
        ),
        PathBuf::from("/System/Library/Fonts/Supplemental/Arial.ttf"),
    ]);
    if let Some(root) = std::env::var_os("SystemRoot") {
        paths.push(PathBuf::from(root).join("Fonts").join("segoeui.ttf"));
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_invalid_and_oversized_fonts_do_not_enter_the_font_database()
    -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("fallback.ttf");
        let mut options = usvg::Options::default();
        assert!(!load_font(&mut options, &path)?);
        std::fs::write(&path, b"not a font")?;
        assert!(!load_font(&mut options, &path)?);
        std::fs::write(&path, vec![0; MAX_FONT_BYTES + 1])?;
        assert!(!load_font(&mut options, &path)?);
        assert!(options.fontdb.faces().next().is_none());
        Ok(())
    }
}
