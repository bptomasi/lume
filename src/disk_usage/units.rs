//! Binary units are a presentation choice; accounting always uses integer bytes.

use anyhow::{Result, bail};

/// Display storage automatically or in one fixed binary unit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Unit {
    /// Choose B, KiB, MiB, GiB or TiB independently for each size.
    #[default]
    Auto,
    /// Display exact bytes.
    Bytes,
    /// Display every size in mebibytes (1,048,576 bytes).
    MiB,
    /// Display every size in gibibytes (1,073,741,824 bytes).
    GiB,
}

impl std::str::FromStr for Unit {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "bytes" | "b" => Ok(Self::Bytes),
            "mib" => Ok(Self::MiB),
            "gib" => Ok(Self::GiB),
            _ => bail!("Storage units: auto, bytes, MiB or GiB"),
        }
    }
}

impl Unit {
    /// Format a byte count without presenting a small nonzero size as zero.
    pub fn format(self, bytes: u64) -> String {
        let (divisor, suffix) = match self {
            Self::Bytes => (1, "B"),
            Self::MiB => (1 << 20, "MiB"),
            Self::GiB => (1 << 30, "GiB"),
            Self::Auto => [
                (1 << 40, "TiB"),
                (1 << 30, "GiB"),
                (1 << 20, "MiB"),
                (1 << 10, "KiB"),
                (1, "B"),
            ]
            .into_iter()
            .find(|(divisor, _)| bytes >= *divisor)
            .unwrap_or((1, "B")),
        };
        if divisor == 1 {
            format!("{bytes} B")
        } else if bytes > 0 && u128::from(bytes) * 100 < u128::from(divisor) {
            format!("<0.01 {suffix}")
        } else {
            format!("{:.2} {suffix}", bytes as f64 / divisor as f64)
        }
    }
}
