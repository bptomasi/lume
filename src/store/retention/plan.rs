//! Materialize identities and cutoffs once, without hydrating article bodies.

use anyhow::{Result, ensure};

use super::{Plan, fingerprint};
use crate::{
    config::retention::{Policy, Retention},
    store::Store,
};
use std::sync::atomic::AtomicBool;

impl Store {
    #[cfg(test)]
    pub(crate) fn prepare_retention(
        &self,
        policy: Retention,
        now: i64,
        active_article: Option<i64>,
    ) -> Result<Plan> {
        self.prepare_retention_cancellable(
            policy,
            now,
            active_article,
            &AtomicBool::new(false),
        )
    }

    pub(crate) fn prepare_retention_cancellable(
        &self,
        policy: Retention,
        now: i64,
        active_article: Option<i64>,
        cancel: &AtomicBool,
    ) -> Result<Plan> {
        crate::retention::check_cancel(cancel)?;
        // Planning writes only TEMP tables. A read snapshot keeps large
        // previews from blocking personal edits on another WAL connection.
        let transaction = self.connection.unchecked_transaction()?;
        let version = self.retention_version()?;
        ensure!(
            policy == self.retention_policy()?,
            "Retention policy changed; generate a new preview"
        );
        self.connection
            .execute_batch(include_str!("../sql/retention/prepare.sql"))?;
        if let Some(article) = active_article {
            self.connection
                .prepare_cached(include_str!(
                    "../sql/retention/insert_active.sql"
                ))?
                .execute([article])?;
        }
        if policy.has_limits() {
            self.retention_sources(&policy, now)?;
            if policy.protect_smart_lists {
                self.protect_retention_queries(
                    &policy.unprotected_smart_lists,
                    cancel,
                )?;
            }
            self.plan_candidates(cancel)?;
        }
        let plan = Plan {
            fingerprint: fingerprint(&policy)?,
            policy,
            version,
            upper: self.connection.query_row(
                include_str!("../sql/retention/select_boundary.sql"),
                [],
                |row| row.get(0),
            )?,
        };
        transaction.commit()?;
        Ok(plan)
    }

    fn plan_candidates(&self, cancel: &AtomicBool) -> Result<()> {
        let protection = format!(
            "{}\n{}",
            include_str!("../sql/retention/insert_protected.sql"),
            include_str!("../sql/retention/select_protected.sql")
                .trim()
                .trim_end_matches(';')
        );
        self.connection.execute(&format!("{protection})"), [])?;
        let sql = format!(
            "{}\n{}",
            include_str!("../sql/retention/with_planned_protections.sql"),
            include_str!("../sql/retention/insert_candidates.sql")
        );
        let mut insert = self.connection.prepare_cached(&sql)?;
        let mut after = 0_i64;
        loop {
            crate::retention::check_cancel(cancel)?;
            let upper: Option<i64> = self.connection.query_row(
                include_str!("../sql/retention/select_planning_boundary.sql"),
                [after],
                |row| row.get(0),
            )?;
            let Some(upper) = upper else {
                return Ok(());
            };
            insert.execute(
                rusqlite::named_params! { ":after": after, ":upper": upper },
            )?;
            after = upper;
        }
    }

    fn retention_sources(&self, policy: &Retention, now: i64) -> Result<()> {
        let overrides = policy
            .sources
            .iter()
            .map(|source| {
                (source.id.as_str(), source.resolve(&policy.defaults))
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut select = self.connection.prepare_cached(include_str!(
            "../sql/retention/select_sources.sql"
        ))?;
        let mut insert = self.connection.prepare_cached(include_str!(
            "../sql/retention/insert_source.sql"
        ))?;
        let mut rows = select.query([])?;
        while let Some(row) = rows.next()? {
            let feed: i64 = row.get(0)?;
            let local: Option<String> = row.get(1)?;
            let server: Option<String> = row.get(2)?;
            let remote: Option<i64> = row.get(3)?;
            let key = match (server, remote) {
                (Some(server), Some(remote)) => {
                    format!("server:{server}:{remote}")
                }
                _ => local.unwrap_or_default(),
            };
            let source =
                overrides.get(key.as_str()).unwrap_or(&policy.defaults);
            let [full, image, read, unread] = cutoffs(source, now);
            insert.execute((feed, full, image, read, unread))?;
        }
        Ok(())
    }

    pub(crate) fn retention_counts(&self) -> Result<crate::retention::Report> {
        Ok(self
            .connection
            .prepare_cached(include_str!("../sql/retention/select_counts.sql"))?
            .query_row([], |row| {
                Ok(crate::retention::Report {
                    articles: row.get(0)?,
                    full_texts: row.get(1)?,
                    full_bytes: row.get(2)?,
                    feed_bytes: row.get(3)?,
                    ..Default::default()
                })
            })?)
    }
}

fn cutoffs(policy: &Policy, now: i64) -> [i64; 4] {
    [
        policy.full_text_days,
        policy.image_days,
        policy.read_days,
        policy.unread_days,
    ]
    .map(|age| {
        age.0.map_or(0, |days| {
            now.saturating_sub(i64::from(days) * 86_400).max(0)
        })
    })
}
