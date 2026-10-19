//! Note predicates share the bounded grammar, but every leaf targets one note.
//!
//! Article search intentionally returns articles. This compiler instead keeps
//! tags, type, status and all text terms correlated to the same annotation ID.

use anyhow::{Result, bail, ensure};
use rusqlite::types::Value;

use super::{Expression, compile::Compiled};

/// Validate a note query without touching storage.
///
/// # Errors
/// Rejects unsupported fields, values, dates or expressions over 4 KiB.
pub fn validate(query: &str) -> Result<()> {
    compile(query).map(|_| ())
}

pub(crate) fn compile(query: &str) -> Result<Compiled> {
    ensure!(query.len() <= 4096, "Note search exceeds 4 KiB");
    let expression = super::parse_syntax(query)?;
    let mut values = Vec::new();
    let sql = expression_sql(&expression, &mut values)?;
    Ok(Compiled { sql, values })
}

fn expression_sql(
    expression: &Expression,
    values: &mut Vec<Value>,
) -> Result<String> {
    Ok(match expression {
        Expression::All => "1".into(),
        Expression::And(left, right) => format!(
            "({} AND {})",
            expression_sql(left, values)?,
            expression_sql(right, values)?
        ),
        Expression::Or(left, right) => format!(
            "({} OR {})",
            expression_sql(left, values)?,
            expression_sql(right, values)?
        ),
        Expression::Not(inner) => {
            format!("NOT ({})", expression_sql(inner, values)?)
        }
        Expression::Term {
            field,
            value,
            phrase,
        } => term(field, value, *phrase, values)?,
    })
}

fn bind(values: &mut Vec<Value>, value: impl Into<Value>) -> String {
    values.push(value.into());
    format!("?{}", values.len())
}

fn term(
    field: &str,
    value: &str,
    phrase: bool,
    values: &mut Vec<Value>,
) -> Result<String> {
    Ok(match field {
        "" | "body" | "quote" => text(field, value, phrase, values),
        "article" | "title" | "source" | "author" => {
            article(field, value, phrase, values)
        }
        "after" | "before" | "updated-after" | "updated-before" => {
            date(field, value, values)?
        }
        _ => metadata(field, value, values)?,
    })
}

fn metadata(
    field: &str,
    value: &str,
    values: &mut Vec<Value>,
) -> Result<String> {
    Ok(match field {
        "tag" => {
            let parameter = bind(values, value.to_owned());
            format!(
                r#"EXISTS (
                    SELECT 1
                    FROM comment_tags AS t
                    WHERE t.comment_id = c.id
                        AND t.tag = {parameter} COLLATE NOCASE
                )"#
            )
        }
        "type" => {
            let kind = crate::model::NoteKind::named(value)?;
            format!("m.kind = {}", bind(values, kind.name().to_owned()))
        }
        "status" => {
            ensure!(
                matches!(value, "open" | "resolved"),
                "Use status:open or status:resolved"
            );
            format!("m.status = {}", bind(values, value.to_owned()))
        }
        "is" => state(value)?,
        "id" => format!("c.id = {}", bind(values, value.parse::<i64>()?)),
        "ref" => format!(
            r#"EXISTS (
                SELECT 1
                FROM note_references
                WHERE source_id = c.id AND target_id = {}
            )"#,
            bind(values, value.parse::<i64>()?)
        ),
        _ => bail!("Unknown note-search field: {field}"),
    })
}

fn state(value: &str) -> Result<String> {
    Ok(match value {
        "due" => r#"m.status = 'open' AND coalesce((
            SELECT due_at FROM annotation_reviews WHERE comment_id = c.id
        ), 0) <= unixepoch()"#
            .into(),
        "pinned" => "m.pinned = 1".into(),
        "highlight" => "c.quote <> ''".into(),
        "general" => "c.quote = ''".into(),
        "linked" => r#"EXISTS (
            SELECT 1 FROM note_references WHERE source_id = c.id
        )"#
        .into(),
        _ => bail!(
            "Use is:due, is:pinned, is:highlight, is:general or is:linked"
        ),
    })
}

fn literal(value: &str, phrase: bool) -> String {
    let suffix = if phrase { "" } else { "*" };
    format!("\"{}\"{}", value.replace('"', "\"\""), suffix)
}

fn text(
    field: &str,
    value: &str,
    phrase: bool,
    values: &mut Vec<Value>,
) -> String {
    let literal = literal(value, phrase);
    let query = if field.is_empty() {
        literal
    } else {
        format!("{field} : {literal}")
    };
    let parameter = bind(values, query);
    format!(
        r#"c.id IN (
            SELECT rowid FROM note_search
            WHERE note_search MATCH {parameter}
        )"#
    )
}

fn article(
    field: &str,
    value: &str,
    phrase: bool,
    values: &mut Vec<Value>,
) -> String {
    if field == "article"
        && let Ok(id) = value.parse::<i64>()
    {
        return format!("c.article_id = {}", bind(values, id));
    }
    let column = match field {
        "source" => "feed",
        "author" => "author",
        _ => "title",
    };
    let query = format!("{column} : {}", literal(value, phrase));
    format!(
        r#"c.article_id IN (
            SELECT rowid FROM article_search
            WHERE article_search MATCH {}
        )"#,
        bind(values, query)
    )
}

fn date(field: &str, value: &str, values: &mut Vec<Value>) -> Result<String> {
    let timestamp = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")?
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow::anyhow!("Invalid note date"))?
        .and_utc()
        .timestamp();
    let column = if field.starts_with("updated-") {
        "m.updated_at"
    } else {
        "c.created"
    };
    let operator = if field.ends_with("before") { "<" } else { ">=" };
    Ok(format!("{column} {operator} {}", bind(values, timestamp)))
}

/// Complete a note-query field prefix without changing other criteria.
pub fn complete(query: &str) -> Option<String> {
    const FIELDS: &[&str] = &[
        "tag:",
        "type:",
        "status:",
        "article:",
        "source:",
        "body:",
        "quote:",
        "author:",
        "after:",
        "before:",
        "updated-after:",
        "updated-before:",
        "is:due",
        "is:pinned",
        "is:highlight",
        "is:general",
        "is:linked",
        "id:",
        "ref:",
    ];
    let start = query
        .rfind(char::is_whitespace)
        .map_or(0, |index| index + 1);
    let prefix = &query[start..];
    let field = FIELDS
        .iter()
        .find(|field| field.starts_with(prefix) && **field != prefix)?;
    Some(format!("{}{field}", &query[..start]))
}

/// Positive text leaves supply SQLite snippets using its own tokenizer.
pub(crate) fn preview_query(query: &str) -> Result<Option<String>> {
    let expression = super::parse_syntax(query)?;
    let mut terms = Vec::new();
    preview_terms(&expression, &mut terms);
    Ok((!terms.is_empty()).then(|| terms.join(" OR ")))
}

fn preview_terms(expression: &Expression, terms: &mut Vec<String>) {
    match expression {
        Expression::And(left, right) | Expression::Or(left, right) => {
            preview_terms(left, terms);
            preview_terms(right, terms);
        }
        Expression::Term {
            field,
            value,
            phrase,
        } if matches!(field.as_str(), "" | "body" | "quote") => {
            let value = literal(value, *phrase);
            terms.push(if field.is_empty() {
                value
            } else {
                format!("{field} : {value}")
            });
        }
        _ => {}
    }
}
