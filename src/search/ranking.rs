//! Relevance hints never change membership or expand saved-list criteria.
//!
//! Only the interactive expression contributes. Positive title terms retain
//! their boolean structure; exclusions and metadata filters still belong to
//! the membership compiler. A plain conjunction also gets phrase/start boosts.

use super::{Expression, compile::fts_literal};

pub(crate) struct Ranking {
    pub(crate) text: String,
    pub(crate) title: Option<String>,
    pub(crate) phrase: Option<String>,
    pub(crate) prefix: Option<String>,
}

impl Ranking {
    pub(crate) fn new(
        expression: &Expression,
        publisher: bool,
    ) -> Option<Self> {
        let mut text = positive(expression, false)?;
        if publisher {
            text = format!("{{title feed author body}} : ({text})");
        }
        let title = positive(expression, true)
            .map(|terms| format!("title : ({terms})"));
        let mut words = Vec::new();
        let phrase = if phrase_words(expression, &mut words) {
            words.last().map(|(_, last_phrase)| {
                let value = words
                    .iter()
                    .map(|(value, _)| *value)
                    .collect::<Vec<_>>()
                    .join(" ");
                fts_literal(&value, *last_phrase)
            })
        } else {
            None
        };
        Some(Self {
            prefix: phrase.as_ref().map(|text| format!("title : ^ {text}")),
            phrase: phrase.map(|text| format!("title : {text}")),
            text,
            title,
        })
    }
}

fn positive(expression: &Expression, title_only: bool) -> Option<String> {
    match expression {
        Expression::Term {
            field,
            value,
            phrase,
        } => {
            let literal = fts_literal(value, *phrase);
            match field.as_str() {
                "" | "topic" | "title" if title_only => Some(literal),
                _ if title_only => None,
                "" | "topic" => Some(literal),
                "source" => Some(format!("feed : {literal}")),
                "title" | "author" | "body" | "notes" => {
                    Some(format!("{field} : {literal}"))
                }
                _ => None,
            }
        }
        Expression::And(left, right) | Expression::Or(left, right) => {
            let operator =
                if title_only && matches!(expression, Expression::And(_, _)) {
                    "AND"
                } else {
                    "OR"
                };
            combine(
                positive(left, title_only),
                positive(right, title_only),
                operator,
            )
        }
        Expression::All | Expression::Not(_) => None,
    }
}

fn combine(
    left: Option<String>,
    right: Option<String>,
    operator: &str,
) -> Option<String> {
    match (left, right) {
        (Some(left), Some(right)) => {
            Some(format!("({left} {operator} {right})"))
        }
        (Some(text), None) | (None, Some(text)) => Some(text),
        (None, None) => None,
    }
}

fn phrase_words<'a>(
    expression: &'a Expression,
    words: &mut Vec<(&'a str, bool)>,
) -> bool {
    match expression {
        Expression::Term {
            field,
            value,
            phrase,
        } if matches!(field.as_str(), "" | "topic" | "title") => {
            words.push((value, *phrase));
            true
        }
        Expression::And(left, right) => {
            phrase_words(left, words) && phrase_words(right, words)
        }
        _ => false,
    }
}
