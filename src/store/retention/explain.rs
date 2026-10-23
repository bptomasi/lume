//! Explicit explanation uses the same authoritative protection predicates as apply.
use super::{Store, protected_query};
use anyhow::{Context, Result, ensure};

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
        let plan = self.prepare_retention_cancellable(
            self.retention_policy()?,
            now,
            None,
            &std::sync::atomic::AtomicBool::new(false),
        )?;
        let sql = protected_query(include_str!(
            "../sql/retention/explain_reasons.sql"
        ));
        let reasons = self
            .connection
            .prepare_cached(&sql)?
            .query_map([article], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let explanation = self
            .connection
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
            .context("Article is missing from the library")?;
        ensure!(
            self.retention_current(&plan)?,
            "Library changed during inspection; inspect again"
        );
        Ok(explanation.text(article, &reasons))
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
