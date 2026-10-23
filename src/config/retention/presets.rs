//! Presets expand to ordinary editable values; no hidden policy survives them.

use super::{Age, Policy, Retention};
use anyhow::{Result, bail};

/// Starting points preserve unread articles and every personal protection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Keep all article and cache content.
    Archive,
    /// Expire recoverable content after thirty days, retaining every article.
    CacheOnly,
    /// Thirty-day caches and ninety-day read articles.
    Balanced,
    /// Seven-day Full text, fourteen-day images and thirty-day read articles.
    Compact,
}

impl std::str::FromStr for Preset {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> Result<Self> {
        match name {
            "archive" => Ok(Self::Archive),
            "cache-only" => Ok(Self::CacheOnly),
            "balanced" => Ok(Self::Balanced),
            "compact" => Ok(Self::Compact),
            _ => bail!("Choose archive, cache-only, balanced or compact"),
        }
    }
}

impl Preset {
    /// Replace global ages and pause automatic maintenance. Source overrides,
    /// protections and transaction/work limits retain the user's choices.
    pub fn apply(self, retention: &mut Retention) {
        let (full, image, read) = match self {
            Self::Archive => (None, None, None),
            Self::CacheOnly => (Some(30), Some(30), None),
            Self::Balanced => (Some(30), Some(30), Some(90)),
            Self::Compact => (Some(7), Some(14), Some(30)),
        };
        retention.defaults = Policy {
            full_text_days: Age(full),
            image_days: Age(image),
            read_days: Age(read),
            unread_days: Age(None),
        };
        retention.automatic = false;
    }
}

/// The same explanation is available from the CLI and the internal command.
pub const GUIDE: &str = "Retention presets (days; unlimited means keep)\n\n\
Preset       Full    Images    Read articles    Unread\n\
archive      keep    keep      keep             keep\n\
cache-only   30      30        keep             keep\n\
balanced     30      30        90               keep\n\
compact      7       14        30               keep\n\n\
Use :retention preset NAME (CLI: lume retention preset NAME).\n\
This replaces global ages in TOML and pauses automatic maintenance.\n\
Per-source overrides, protections and run limits are preserved.\n\
Edit every generated value freely. Preview before applying.\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_preserve_limits_sources_and_protections_and_keep_unread_articles()
    -> Result<()> {
        let mut policy = Retention {
            automatic: true,
            batch_size: 17,
            max_articles_per_run: 41,
            unprotected_smart_lists: vec!["temporary".into()],
            sources: vec![super::super::Source {
                id: "personal".into(),
                full_text_days: None,
                image_days: Some(Age(None)),
                read_days: Some(Age(None)),
                unread_days: None,
            }],
            ..Default::default()
        };
        let sources = policy.sources.clone();
        for name in ["archive", "cache-only", "balanced", "compact"] {
            name.parse::<Preset>()?.apply(&mut policy);
            assert!(!policy.automatic);
            assert_eq!(policy.defaults.unread_days, Age(None));
            assert_eq!(policy.sources, sources);
            assert_eq!(policy.batch_size, 17);
            assert_eq!(policy.max_articles_per_run, 41);
            assert!(policy.protect_smart_lists);
            assert_eq!(policy.unprotected_smart_lists, ["temporary"]);
        }
        Ok(())
    }
}
