//! Explicit explanation uses the same authoritative protection predicates as apply.
use super::Store;
use anyhow::{Context, Result, ensure};
use std::sync::atomic::AtomicBool;

struct Explanation {
    title: String,
    source: String,
    unread: bool,
    eligibility: [(bool, i64); 3],
}

impl Store {
    /// Explain retained personal data and effective expiry eligibility for one article.
    ///
    /// # Errors
    /// Returns missing article, invalid policy or planning/storage failures.
    pub fn explain_retention(&self, article: i64, now: i64) -> Result<String> {
        self.explain_retention_cancellable(
            article,
            now,
            None,
            &AtomicBool::new(false),
        )
    }

    /// Inspect one article without global candidate materialization or policy acknowledgement.
    ///
    /// # Errors
    /// Returns cancellation, missing article, invalid policy or storage failures.
    pub fn explain_retention_cancellable(
        &self,
        article: i64,
        now: i64,
        active: Option<i64>,
        cancel: &AtomicBool,
    ) -> Result<String> {
        let plan = self.prepare_retention_scope(
            self.retention_policy()?,
            now,
            None,
            cancel,
            Some(article),
        )?;
        self.read_snapshot(|store| {
            let mut reasons = store.retention_reasons(article)?;
            if active == Some(article) {
                reasons.push("active reader".into());
            }
            let explanation = store.retention_explanation(article)?;
            crate::retention::check_cancel(cancel)?;
            ensure!(
                store.retention_current(&plan)?,
                "Library changed during inspection; inspect again"
            );
            Ok(explanation.text(article, &reasons))
        })
    }

    fn retention_reasons(&self, article: i64) -> Result<Vec<String>> {
        // Allow the article predicate to reach each canonical protection branch.
        let sql = format!(
            "{}\n{})\n{}",
            include_str!("../sql/retention/with_inspection_protections.sql"),
            include_str!("../sql/retention/select_protected.sql")
                .trim()
                .trim_end_matches(';'),
            include_str!("../sql/retention/explain_reasons.sql")
        );
        self.connection
            .prepare_cached(&sql)?
            .query_map([article], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn retention_explanation(&self, article: i64) -> Result<Explanation> {
        self.connection
            .query_row(
                include_str!("../sql/retention/explain_article.sql"),
                [article],
                |row| {
                    Ok(Explanation {
                        title: row.get(0)?,
                        source: row.get(1)?,
                        unread: row.get(2)?,
                        eligibility: [
                            (row.get(3)?, row.get(6)?),
                            (row.get(4)?, row.get(7)?),
                            (row.get(5)?, row.get(8)?),
                        ],
                    })
                },
            )
            .context("Article is missing from the library")
    }
}

impl Explanation {
    fn text(&self, article: i64, reasons: &[String]) -> String {
        let mut text = format!(
            "Article {article}: {}\nSource: {}\nState: {}\n\n",
            self.title,
            self.source,
            if self.unread { "Unread" } else { "Read" }
        );
        for (name, (eligible, cutoff)) in
            ["Article", "Full cache", "Image references"]
                .into_iter()
                .zip(self.eligibility)
        {
            let status = if !reasons.is_empty() {
                "protected"
            } else if eligible {
                "eligible after preview, if present"
            } else if cutoff == 0 {
                "no active expiry cutoff"
            } else {
                "age boundary not reached or content absent"
            };
            let cutoff = chrono::DateTime::from_timestamp(cutoff, 0)
                .filter(|_| cutoff > 0)
                .map(|date| date.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "none".into());
            text.push_str(&format!("{name}: {status} · cutoff {cutoff}\n"));
        }
        if reasons.is_empty() {
            text.push_str("\nNo personal protection matches this article.\n");
        } else {
            text.push_str("\nProtected by:\n");
            for reason in reasons {
                text.push_str(&format!("- {reason}\n"));
            }
        }
        text.push_str(
            "\nInspection never acknowledges a policy or removes content.\n",
        );
        text
    }
}
