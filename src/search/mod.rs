//! Bounded domain search grammar composed with nom, compiled to bound SQL.
//!
//! NOT binds more tightly than AND; AND binds more tightly than OR. Adjacent
//! terms imply AND. Quoted terms are exact FTS phrases; bare terms are prefixes.
//! This module never interpolates a user value or identifier into SQL syntax.

pub(crate) mod compile;
mod grammar;
pub mod notes;
pub(crate) mod ranking;

pub(crate) use compile::compile;

use anyhow::{Result, ensure};

/// Validated search expression, shared by live search and saved lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expression {
    /// Unrestricted empty search.
    All,
    /// A field-qualified literal or phrase.
    Term {
        /// Validated domain field name; empty means full text.
        field: String,
        /// Decoded literal value.
        value: String,
        /// Match an exact phrase instead of a word prefix.
        phrase: bool,
    },
    /// Both expressions must match.
    And(Box<Expression>, Box<Expression>),
    /// Either expression may match.
    Or(Box<Expression>, Box<Expression>),
    /// Exclude the nested expression.
    Not(Box<Expression>),
}

impl Expression {
    /// Whether changing read state can alter the expression's membership.
    pub(crate) fn depends_on_read_state(&self) -> bool {
        match self {
            Self::All => false,
            Self::Term { field, value, .. } => {
                field == "is" && matches!(value.as_str(), "read" | "unread")
            }
            Self::And(left, right) | Self::Or(left, right) => {
                left.depends_on_read_state() || right.depends_on_read_state()
            }
            Self::Not(inner) => inner.depends_on_read_state(),
        }
    }
}

/// Parse a bounded query; invalid expressions never become unrestricted searches.
///
/// # Errors
/// Returns syntax, depth, token-count or supported-field validation errors.
pub fn parse(query: &str) -> Result<Expression> {
    let expression = parse_syntax(query)?;
    // Compile now as well so invalid dates/ranges/states fail before saving.
    compile(&expression)?;
    Ok(expression)
}

pub(crate) fn parse_syntax(query: &str) -> Result<Expression> {
    ensure!(query.len() <= 16_384, "Search exceeds 16 KiB");
    grammar::parse(query)
}

/// Explain precedence and token meaning without hiding active criteria.
///
/// # Errors
/// Returns the same syntax and field validation failures as `parse`.
pub fn explain(query: &str) -> Result<String> {
    Ok(format!(
        "{}\n\nNOT binds before AND; AND before OR.\n\
        Phrases match complete token sequences; bare terms match prefixes.",
        describe(&parse(query)?)
    ))
}

/// Complete a field/operator prefix without changing already typed criteria.
pub fn suggestions(prefix: &str) -> Vec<&'static str> {
    const TOKENS: &[&str] = &[
        "title:",
        "author:",
        "source:",
        "body:",
        "notes:",
        "domain:",
        "topic:",
        "highlight-tag:",
        "tag:",
        "after:",
        "before:",
        "minutes:",
        "is:unread",
        "is:read",
        "is:favorite",
        "is:queued",
        "is:annotated",
        "is:cached",
        "AND",
        "OR",
        "NOT",
    ];
    TOKENS
        .iter()
        .copied()
        .filter(|token| token.starts_with(prefix))
        .collect()
}

/// Complete the final field/operator prefix, respecting UTF-8 whitespace.
pub fn complete(query: &str) -> Option<String> {
    let start = query
        .char_indices()
        .rfind(|(_, c)| c.is_whitespace() || *c == '(')
        .map_or(0, |(index, c)| index + c.len_utf8());
    let choices = suggestions(&query[start..]);
    let choice = choices.first()?;
    Some(format!("{}{choice}", &query[..start]))
}

fn describe(expression: &Expression) -> String {
    match expression {
        Expression::All => "All articles".into(),
        Expression::And(left, right) => {
            format!("({}) AND ({})", describe(left), describe(right))
        }
        Expression::Or(left, right) => {
            format!("({}) OR ({})", describe(left), describe(right))
        }
        Expression::Not(inner) => format!("Exclude ({})", describe(inner)),
        Expression::Term {
            field,
            value,
            phrase,
        } => describe_term(field, value, *phrase),
    }
}

fn describe_term(field: &str, value: &str, phrase: bool) -> String {
    match field {
        "is" => format!("State: {value}"),
        "tag" => format!("Article tag: {value}"),
        "highlight-tag" => format!("Annotation tag: {value}"),
        "domain" => format!("Original domain: {value}"),
        "after" => format!("Published on/after {value} UTC"),
        "before" => format!("Published before {value} UTC"),
        "minutes" => format!("Estimated reading time: {value} minutes"),
        _ => {
            let label = if field.is_empty() || field == "topic" {
                "Full text"
            } else {
                field
            };
            let kind = if phrase {
                "exact phrase"
            } else {
                "word prefix"
            };
            format!("{label}: {kind} \"{value}\"")
        }
    }
}
