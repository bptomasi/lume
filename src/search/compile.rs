//! Domain fields compile only to owned SQL fragments and numbered bound values.

use anyhow::{Result, bail, ensure};
use rusqlite::types::Value;

use super::Expression;

pub(crate) struct Compiled {
    pub(crate) sql: String,
    pub(crate) values: Vec<Value>,
}

pub(crate) fn compile(expression: &Expression) -> Result<Compiled> {
    compile_scope(expression, true, None)
}

pub(crate) fn compile_publisher(expression: &Expression) -> Result<Compiled> {
    compile_scope(expression, false, None)
}

fn compile_scope(
    expression: &Expression,
    include_notes: bool,
    row_range: Option<(i64, i64)>,
) -> Result<Compiled> {
    let mut values = Vec::new();
    let sql =
        expression_sql(expression, &mut values, include_notes, row_range)?;
    Ok(Compiled { sql, values })
}

fn expression_sql(
    expression: &Expression,
    values: &mut Vec<Value>,
    include_notes: bool,
    row_range: Option<(i64, i64)>,
) -> Result<String> {
    Ok(match expression {
        Expression::All => "1".into(),
        Expression::And(left, right) => format!(
            "({} AND {})",
            expression_sql(left, values, include_notes, row_range)?,
            expression_sql(right, values, include_notes, row_range)?
        ),
        Expression::Or(left, right) => format!(
            "({} OR {})",
            expression_sql(left, values, include_notes, row_range)?,
            expression_sql(right, values, include_notes, row_range)?
        ),
        Expression::Not(inner) => {
            format!(
                "NOT ({})",
                expression_sql(inner, values, include_notes, row_range)?
            )
        }
        Expression::Term {
            field,
            value,
            phrase,
        } => term(field, value, *phrase, values, include_notes, row_range)?,
    })
}

fn bind(values: &mut Vec<Value>, value: impl Into<Value>) -> String {
    let name = format!(":search_{}", values.len());
    values.push(value.into());
    name
}

fn term(
    field: &str,
    value: &str,
    phrase: bool,
    values: &mut Vec<Value>,
    include_notes: bool,
    row_range: Option<(i64, i64)>,
) -> Result<String> {
    match field {
        "notes" | "highlight-tag" if !include_notes => {
            bail!(
                "For you searches publisher content; open Notebook to search notes"
            )
        }
        "" | "title" | "author" | "source" | "body" | "notes" | "topic" => Ok(
            full_text(field, value, phrase, values, include_notes, row_range),
        ),
        "is" => state(value),
        "after" | "before" => date(field, value, values),
        "domain" => domain(value, values),
        "minutes" => minutes(value, values),
        "tag" => {
            let parameter = bind(values, value.to_owned());
            Ok(format!(
                r#"EXISTS (
                    SELECT 1 FROM article_tags
                    WHERE article_id = a.id
                        AND tag = {parameter} COLLATE NOCASE
                )"#
            ))
        }
        "highlight-tag" => {
            let parameter = bind(values, value.to_owned());
            Ok(format!(
                r#"EXISTS (
                SELECT 1
                FROM comments AS c
                JOIN comment_tags AS t ON t.comment_id = c.id
                WHERE c.article_id = a.id
                    AND t.tag = {parameter} COLLATE NOCASE
            )"#
            ))
        }
        _ => bail!("Unknown search field: {field}"),
    }
}

fn full_text(
    field: &str,
    value: &str,
    phrase: bool,
    values: &mut Vec<Value>,
    include_notes: bool,
    row_range: Option<(i64, i64)>,
) -> String {
    let literal = fts_literal(value, phrase);
    let query = match field {
        "" | "topic" => literal.clone(),
        "source" => format!("feed : {literal}"),
        _ => format!("{field} : {literal}"),
    };
    if field == "notes" {
        return matching_notes(values, &literal);
    }
    let parameter = bind(values, query);
    let row = publisher_row_bound(include_notes, row_range, values);
    let article_match = format!(
        r#"a.id IN (
            SELECT rowid FROM article_search
            WHERE article_search MATCH {parameter}{row}
        )"#
    );
    if include_notes && matches!(field, "" | "topic") {
        return format!(
            "({article_match} OR {})",
            matching_notes(values, &literal)
        );
    }
    article_match
}

pub(super) fn fts_literal(value: &str, phrase: bool) -> String {
    format!(
        "\"{}\"{}",
        value.replace('"', "\"\""),
        if phrase { "" } else { "*" }
    )
}

fn matching_notes(values: &mut Vec<Value>, literal: &str) -> String {
    let parameter = bind(values, literal.to_owned());
    format!(
        r#"a.id IN (
            SELECT c.article_id
            FROM note_search
            JOIN comments AS c ON c.id = note_search.rowid
            WHERE note_search MATCH {parameter}
        )"#
    )
}

fn state(value: &str) -> Result<String> {
    Ok(match value {
        "unread" => "a.unread = 1",
        "read" => "a.unread = 0",
        "favorite" => "a.starred = 1",
        "queued" => "a.queued = 1",
        "annotated" => {
            "EXISTS (SELECT 1 FROM comments WHERE article_id = a.id)"
        }
        "cached" => {
            r#"EXISTS (
            SELECT 1 FROM article_documents
            WHERE article_id = a.id AND url = a.url AND body <> ''
        )"#
        }
        _ => bail!("Unknown article state: {value}"),
    }
    .into())
}

fn date(field: &str, value: &str, values: &mut Vec<Value>) -> Result<String> {
    let date = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")?;
    let timestamp = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow::anyhow!("Invalid date"))?
        .and_utc()
        .timestamp();
    let parameter = bind(values, timestamp);
    Ok(format!(
        "(a.published > 0 AND a.published {} {parameter})",
        if field == "after" { ">=" } else { "<" }
    ))
}

fn domain(value: &str, values: &mut Vec<Value>) -> Result<String> {
    let url = crate::source::feed_url(&format!("https://{value}/"))?;
    ensure!(
        url.host_str() == Some(value) && url.port().is_none(),
        "Use a lowercase hostname without a path"
    );
    let https = bind(values, format!("https://{value}"));
    let http = bind(values, format!("http://{value}"));
    // Canonical keys are URL-library validated and normalize hostname casing.
    // A boundary keeps suffix hosts out while admitting non-default ports.
    Ok(format!(
        r#"(
            (
                substr(a.canonical_key, 1, length({https})) = {https}
                AND substr(a.canonical_key, length({https}) + 1, 1)
                    IN ('/', ':')
            ) OR (
                substr(a.canonical_key, 1, length({http})) = {http}
                AND substr(a.canonical_key, length({http}) + 1, 1)
                    IN ('/', ':')
            )
        )"#
    ))
}

fn minutes(value: &str, values: &mut Vec<Value>) -> Result<String> {
    let (minimum, maximum) = value.split_once("..").unwrap_or((value, value));
    let minimum: i64 = minimum.parse()?;
    let maximum: i64 = maximum.parse()?;
    ensure!(
        minimum >= 0 && minimum <= maximum && maximum <= 10_000,
        "Invalid minutes range"
    );
    let minimum = bind(values, minimum);
    let maximum = bind(values, maximum);
    Ok(format!("a.reading_minutes BETWEEN {minimum} AND {maximum}"))
}

fn publisher_row_bound(
    include_notes: bool,
    row_range: Option<(i64, i64)>,
    values: &mut Vec<Value>,
) -> String {
    if let Some((first, last)) = row_range {
        let first = bind(values, first);
        let last = bind(values, last);
        return format!(" AND rowid BETWEEN {first} AND {last}");
    }
    if include_notes {
        return String::new();
    }
    " AND rowid = a.id".into()
}

pub(crate) fn compile_publisher_range(
    expression: &Expression,
    first: i64,
    last: i64,
) -> Result<Compiled> {
    // A bounded range parses each FTS leaf once for the batch. Sparse identity
    // sets retain correlated lookups rather than scanning unrelated documents.
    compile_scope(expression, false, Some((first, last)))
}
