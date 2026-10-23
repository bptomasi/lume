//! Unlimited defaults and explicit, portable retention overrides.

use anyhow::{Result, ensure};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A positive number of days or the explicit TOML string `"unlimited"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Age(pub Option<u32>);

impl Serialize for Age {
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match self.0 {
            Some(days) => serializer.serialize_u32(days),
            None => serializer.serialize_str("unlimited"),
        }
    }
}

impl<'de> Deserialize<'de> for Age {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Value {
            Days(u32),
            Text(String),
        }
        match Value::deserialize(decoder)? {
            Value::Days(days) if (1..=365_000).contains(&days) => {
                Ok(Self(Some(days)))
            }
            Value::Text(value) if value == "unlimited" => Ok(Self(None)),
            _ => Err(serde::de::Error::custom(
                "Retention age must be 1–365,000 days or 'unlimited'",
            )),
        }
    }
}

/// Independent recoverable-cache and article lifetimes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    /// Age since successful Full download; feed text remains intact.
    pub full_text_days: Age,
    /// Age since article admission for ordinary image references.
    pub image_days: Age,
    /// Age since the later of admission and last reading-state change.
    pub read_days: Age,
    /// Age since the later of admission and last reading-state change.
    pub unread_days: Age,
}

/// A source inherits omitted fields and can explicitly restore unlimited age.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Local feed ID or `server:CONNECTION:REMOTE_FEED_ID`.
    pub id: String,
    /// Override the global Full cache lifetime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_text_days: Option<Age>,
    /// Override the ordinary image lifetime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_days: Option<Age>,
    /// Override read article lifetime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_days: Option<Age>,
    /// Override unread article lifetime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unread_days: Option<Age>,
}

impl Source {
    pub(crate) fn resolve(&self, base: &Policy) -> Policy {
        Policy {
            full_text_days: self.full_text_days.unwrap_or(base.full_text_days),
            image_days: self.image_days.unwrap_or(base.image_days),
            read_days: self.read_days.unwrap_or(base.read_days),
            unread_days: self.unread_days.unwrap_or(base.unread_days),
        }
    }
}

/// Background maintenance is opt-in and requires preview of the same policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Retention {
    /// Apply an already previewed policy in the background.
    pub automatic: bool,
    /// Minimum interval between automatic passes, persisted across restarts.
    pub interval_hours: u16,
    /// Maximum articles changed in one SQLite transaction.
    pub batch_size: u16,
    /// Maximum article actions in one pass; subsequent passes resume remaining work.
    pub max_articles_per_run: u32,
    /// Maximum ordinary image files removed in one pass.
    pub max_images_per_run: u32,
    /// Protect matches of every active smart list in addition to personal folders.
    pub protect_smart_lists: bool,
    /// Temporary smart lists whose matches may expire; personal folders stay protected.
    pub unprotected_smart_lists: Vec<String>,
    /// Fallback lifetimes for sources without an override.
    pub defaults: Policy,
    /// Independent field overrides by portable source identity.
    pub sources: Vec<Source>,
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            automatic: false,
            interval_hours: 24,
            batch_size: 32,
            max_articles_per_run: 1000,
            max_images_per_run: 1000,
            protect_smart_lists: true,
            unprotected_smart_lists: Vec::new(),
            defaults: Policy::default(),
            sources: Vec::new(),
        }
    }
}

impl Policy {
    fn ages(&self) -> [Age; 4] {
        [
            self.full_text_days,
            self.image_days,
            self.read_days,
            self.unread_days,
        ]
    }

    fn validate(&self) -> Result<()> {
        for age in self.ages() {
            ensure!(
                age.0.is_none_or(|days| (1..=365_000).contains(&days)),
                "Invalid retention age"
            );
        }
        Ok(())
    }
}

impl Retention {
    pub(crate) fn has_limits(&self) -> bool {
        let limited =
            |policy: &Policy| policy.ages().iter().any(|age| age.0.is_some());
        limited(&self.defaults)
            || self
                .sources
                .iter()
                .any(|source| limited(&source.resolve(&self.defaults)))
    }

    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            (1..=8760).contains(&self.interval_hours),
            "Retention interval must be 1–8,760 hours"
        );
        ensure!(
            (1..=256).contains(&self.batch_size),
            "Retention batch must be 1–256 articles"
        );
        ensure!(
            (1..=100_000).contains(&self.max_articles_per_run),
            "Retention run limit must be 1–100,000 articles"
        );
        ensure!(
            (1..=100_000).contains(&self.max_images_per_run),
            "Retention image limit must be 1–100,000 files"
        );
        self.defaults.validate()?;
        self.validate_definitions()
    }

    fn validate_definitions(&self) -> Result<()> {
        ensure!(
            self.sources.len() <= 5000,
            "At most 5,000 retention overrides"
        );
        ensure!(
            self.unprotected_smart_lists.len() <= 1000,
            "At most 1,000 unprotected smart lists"
        );
        let unique = self
            .unprotected_smart_lists
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        ensure!(
            unique.len() == self.unprotected_smart_lists.len(),
            "Duplicate unprotected smart list"
        );
        let mut ids = std::collections::BTreeSet::new();
        for source in &self.sources {
            ensure!(ids.insert(&source.id), "Duplicate retention source ID");
            source.resolve(&self.defaults).validate()?;
        }
        Ok(())
    }
}
