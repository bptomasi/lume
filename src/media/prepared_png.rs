//! Private PNG lifetime follows cached and admitted presentation references.

use std::path::Path;

use anyhow::Result;
use image::{DynamicImage, ImageFormat};

/// A worker-created temporary PNG; dropping the last owner deletes its file.
pub struct PreparedPng {
    file: tempfile::NamedTempFile,
    bytes: u64,
}

impl PreparedPng {
    pub(crate) fn new(image: &DynamicImage) -> Result<Self> {
        let mut file = tempfile::Builder::new()
            .prefix("lume-image-")
            .suffix(".png")
            .tempfile()?;
        image.write_to(&mut file, ImageFormat::Png)?;
        let bytes = file.as_file().metadata()?.len();
        Ok(Self { file, bytes })
    }

    /// Local, privately created file; no publisher string controls this path.
    pub fn path(&self) -> &Path {
        self.file.path()
    }

    /// Encoded bytes counted against the prepared image cache budget.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}
