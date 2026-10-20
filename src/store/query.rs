//! SQLite-backed view pagination and parameterized boolean full-text search.
//!
//! Feed/list/favorite/notebook views query all stored articles. Only a bounded
//! page reaches application memory. FTS expressions are composed from quoted
//! literal leaves; the domain AST owns operators and never interpolates SQL.

use anyhow::{Context, Result, ensure};
use rusqlite::{Statement, ToSql};

use crate::model::{ArticlePage, ArticleQuery, SmartFilter, View};

use super::{Store, article_row};

struct Criteria {
    scope: &'static str,
    identity: i64,
    filter: SmartFilter,
    since: i64,
    feeds: String,
    expression: crate::search::compile::Compiled,
}

impl Criteria {
    fn position_sql(&self) -> &'static str {
        if self.scope == "recommended" {
            return include_str!(
                "sql/recommendations/select_cached_position.sql"
            );
        }
        include_str!("sql/select_matching_position.sql")
    }

    fn page_sql(&self, predicate: &str) -> String {
        if self.scope == "recommended" {
            return format!(
                "WITH matching AS ({predicate})\n{}",
                include_str!("sql/recommendations/select_cached_page.sql")
            );
        }
        // Materialize only page identities before loading metadata or counting
        // comments. Sorting a large result must not hydrate every article.
        format!(
            r#"WITH candidates AS (
                {predicate}
            ),
            matching AS MATERIALIZED (
                {}
            )
            {}
            {}"#,
            include_str!("sql/select_matching_page.sql"),
            include_str!("sql/select_article_metadata.sql"),
            include_str!("sql/select_page.sql")
        )
    }

    fn predicate(&self) -> String {
        let mut conditions = vec![scope_clause(self.scope)];
        if self.filter.unread {
            conditions.push("a.unread = 1");
        }
        if self.filter.starred {
            conditions.push("a.starred = 1");
        }
        if self.filter.feed.is_some() {
            conditions.push("a.feed_id = :feed");
        }
        if !self.filter.feeds.is_empty() {
            conditions
                .push("a.feed_id IN (SELECT value FROM json_each(:feeds))");
        }
        if self.filter.days.is_some() {
            conditions.push("a.published >= :since");
        }
        let base = if self.scope == "recommended" {
            include_str!("sql/recommendations/select_cached_matching.sql")
        } else {
            include_str!("sql/select_matching.sql")
        };
        let matching = format!(
            "{}\nAND ({})\nAND ({})",
            base,
            conditions.join(")\nAND ("),
            self.expression.sql
        );
        if self.scope != "grouped" {
            return matching;
        }
        format!(
            "WITH candidates AS ({matching})\n{}",
            include_str!("sql/select_grouped_candidates.sql")
        )
    }
}

impl Store {
    /// Read one article, including Trash and archived subscriptions.
    ///
    /// # Errors
    /// Returns missing article or SQLite errors.
    pub fn article(&self, article: i64) -> Result<crate::model::Article> {
        let sql = format!(
            "{}\nWHERE a.id = ?1",
            include_str!("sql/select_article_metadata.sql")
        );
        Ok(self.connection.query_row(&sql, [article], article_row)?)
    }

    /// Locate an article in global newest-first order for palette/session jumps.
    ///
    /// # Errors
    /// Returns missing/deleted article or SQLite errors.
    pub fn article_position(&self, article: i64) -> Result<usize> {
        self.connection
            .query_row(
                include_str!("sql/select_article_position.sql"),
                [article],
                |row| row.get(0),
            )
            .context(
                "Article is missing or in Trash; restore it before jumping",
            )
    }

    /// Query a bounded metadata page across the entire library.
    ///
    /// # Errors
    /// Returns invalid smart-list or SQLite errors, preserving existing data.
    pub fn query_articles(&self, query: &ArticleQuery) -> Result<ArticlePage> {
        // Count and page must observe one snapshot while the collector commits.
        self.read_snapshot(|store| {
            let criteria = store.criteria(query)?;
            store.read_page(query, &criteria)
        })
    }

    /// Locate a matching article and load its bounded page in one snapshot.
    ///
    /// # Errors
    /// Returns invalid queries or SQLite errors. A nonmatching target is absent.
    pub fn page_containing(
        &self,
        query: &ArticleQuery,
        article: i64,
    ) -> Result<Option<(usize, ArticlePage)>> {
        self.read_snapshot(|store| store.locate_page(query, article))
    }

    fn locate_page(
        &self,
        query: &ArticleQuery,
        article: i64,
    ) -> Result<Option<(usize, ArticlePage)>> {
        let criteria = self.criteria(query)?;
        let sql = format!(
            "WITH matching AS ({})\n{}",
            criteria.predicate(),
            criteria.position_sql(),
        );
        let mut statement = self.connection.prepare_cached(&sql)?;
        bind(&mut statement, &criteria, query)?;
        let anchor = statement
            .parameter_index(":anchor")?
            .context("Article position query requires its captured identity")?;
        statement.raw_bind_parameter(anchor, article)?;
        let Some(position) = statement
            .raw_query()
            .next()?
            .map(|row| row.get::<_, usize>(0))
            .transpose()?
        else {
            return Ok(None);
        };
        let mut query = query.clone();
        let limit = query.limit.clamp(1, 500);
        query.offset = position / limit * limit;
        let page = self.read_page(&query, &criteria)?;
        Ok(Some((query.offset, page)))
    }

    fn read_page(
        &self,
        query: &ArticleQuery,
        criteria: &Criteria,
    ) -> Result<ArticlePage> {
        let predicate = criteria.predicate();
        let count_sql = format!("SELECT count(*) FROM ({predicate})");
        let mut count = self.connection.prepare_cached(&count_sql)?;
        bind(&mut count, criteria, query)?;
        let total =
            count.raw_query().next()?.map_or(Ok(0), |row| row.get(0))?;
        let page_sql = criteria.page_sql(&predicate);
        let mut page = self.connection.prepare_cached(&page_sql)?;
        bind(&mut page, criteria, query)?;
        let articles = page
            .raw_query()
            .mapped(article_row)
            .collect::<rusqlite::Result<_>>()?;
        Ok(ArticlePage { articles, total })
    }

    /// Count one destination without an arbitrary historical window.
    ///
    /// # Errors
    /// Returns missing smart-list or SQLite errors.
    pub fn view_count(&self, view: &View) -> Result<usize> {
        let query = ArticleQuery {
            view: view.clone(),
            text: String::new(),
            offset: 0,
            limit: 1,
            retained: None,
        };
        let criteria = self.criteria(&query)?;
        let sql = format!("SELECT count(*) FROM ({})", criteria.predicate());
        let mut statement = self.connection.prepare_cached(&sql)?;
        bind(&mut statement, &criteria, &query)?;
        Ok(statement
            .raw_query()
            .next()?
            .map_or(Ok(0), |row| row.get(0))?)
    }

    fn criteria(&self, query: &ArticleQuery) -> Result<Criteria> {
        ensure!(query.text.len() <= 16_384, "Search exceeds 16 KiB");
        let (scope, identity) = scope(&query.view);
        let filter = if let View::Smart(id) = query.view {
            self.smart_filter(id)?
        } else {
            SmartFilter::default()
        };
        let expression = crate::search::Expression::And(
            Box::new(crate::search::parse(&filter.text)?),
            Box::new(crate::search::parse(&query.text)?),
        );
        let expression = if scope == "recommended" {
            crate::search::compile::compile_publisher(&expression)?
        } else {
            crate::search::compile(&expression)?
        };
        let since = filter.days.map_or(0, |days| {
            chrono::Utc::now().timestamp() - i64::from(days) * 86_400
        });
        let feeds = serde_json::to_string(&filter.feeds)?;
        Ok(Criteria {
            scope,
            identity,
            filter,
            since,
            feeds,
            expression,
        })
    }
}

fn scope_clause(scope: &str) -> &'static str {
    match scope {
        "recommended" => include_str!("sql/recommendations/select_scope.sql"),
        "inbox" => "a.unread = 1 OR a.id = :retained",
        "read" => "a.unread = 0",
        "starred" => "a.starred = 1",
        "queue" => "a.queued = 1",
        "feed" => "a.feed_id = :id",
        "since_visit" => ":id > 0 AND a.added_at > :id",
        "continue" => include_str!("sql/select_continue_predicate.sql"),
        "annotated" => {
            "EXISTS (SELECT 1 FROM comments WHERE article_id = a.id)"
        }
        "collection" => include_str!("sql/select_collection_predicate.sql"),
        "all" | "trash" | "grouped" => "1",
        _ => "0",
    }
}

fn scope(view: &View) -> (&'static str, i64) {
    match view {
        View::All | View::Smart(_) => ("all", 0),
        View::Recommended => ("recommended", 0),
        View::Inbox => ("inbox", 0),
        View::Read => ("read", 0),
        View::Trash => ("trash", 0),
        View::Starred => ("starred", 0),
        View::Queue => ("queue", 0),
        View::Annotated => ("annotated", 0),
        View::Feed(id) => ("feed", *id),
        View::Collection(id) => ("collection", *id),
        View::Continue => ("continue", 0),
        View::SinceLastVisit(since) => ("since_visit", *since),
        View::Grouped => ("grouped", 0),
        View::Group(_) => ("group", 0),
    }
}

fn bind(
    statement: &mut Statement<'_>,
    criteria: &Criteria,
    query: &ArticleQuery,
) -> Result<()> {
    let limit = query.limit.clamp(1, 500) as i64;
    let offset = i64::try_from(query.offset)?;
    let values: &[(&str, &dyn ToSql)] = &[
        (":scope", &criteria.scope),
        (":id", &criteria.identity),
        (":unread", &criteria.filter.unread),
        (":starred", &criteria.filter.starred),
        (":feed", &criteria.filter.feed),
        (":feeds", &criteria.feeds),
        (":since", &criteria.since),
        (":retained", &query.retained),
        (":limit", &limit),
        (":offset", &offset),
    ];
    for (name, value) in values {
        // Count queries intentionally omit pagination parameters.
        if let Some(index) = statement.parameter_index(name)? {
            statement.raw_bind_parameter(index, value)?;
        }
    }
    for (index, value) in criteria.expression.values.iter().enumerate() {
        let name = format!(":search_{index}");
        if let Some(parameter) = statement.parameter_index(&name)? {
            statement.raw_bind_parameter(parameter, value)?;
        }
    }
    Ok(())
}
