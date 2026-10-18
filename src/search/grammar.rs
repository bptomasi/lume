//! nom owns lexical decoding; this bounded grammar owns domain precedence.

use anyhow::{Context, Result, bail, ensure};
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{escaped_transform, is_not, take_while1},
    character::complete::{char, multispace0},
    combinator::{all_consuming, map, value},
    multi::many0,
    sequence::{delimited, preceded},
};

use super::Expression;

#[derive(Debug, PartialEq)]
enum Token {
    Word(String),
    Phrase(String),
    Open,
    Close,
}

fn token(input: &str) -> IResult<&str, Token> {
    preceded(
        multispace0,
        alt((
            map(char('('), |_| Token::Open),
            map(char(')'), |_| Token::Close),
            map(
                delimited(
                    char('"'),
                    escaped_transform(
                        is_not("\\\""),
                        '\\',
                        alt((value("\\", char('\\')), value("\"", char('"')))),
                    ),
                    char('"'),
                ),
                Token::Phrase,
            ),
            map(
                take_while1(|c: char| {
                    !c.is_whitespace() && !matches!(c, '(' | ')' | '"')
                }),
                |word: &str| Token::Word(word.to_owned()),
            ),
        )),
    )
    .parse(input)
}

pub(super) fn parse(input: &str) -> Result<Expression> {
    let (_, tokens) =
        all_consuming(delimited(multispace0, many0(token), multispace0))
            .parse(input)
            .map_err(|error| {
                anyhow::anyhow!("Invalid search syntax: {error}")
            })?;
    ensure!(tokens.len() <= 256, "Search exceeds 256 tokens");
    if tokens.is_empty() {
        return Ok(Expression::All);
    }
    let mut cursor = Cursor {
        tokens,
        position: 0,
    };
    let expression = cursor.or(0)?;
    ensure!(
        cursor.position == cursor.tokens.len(),
        "Unexpected closing parenthesis"
    );
    Ok(expression)
}

struct Cursor {
    tokens: Vec<Token>,
    position: usize,
}

impl Cursor {
    fn keyword(&mut self, keyword: &str) -> bool {
        if self.tokens.get(self.position) == Some(&Token::Word(keyword.into()))
        {
            self.position += 1;
            return true;
        }
        false
    }

    fn or(&mut self, depth: usize) -> Result<Expression> {
        let mut expression = self.and(depth)?;
        while self.keyword("OR") {
            expression = Expression::Or(
                Box::new(expression),
                Box::new(self.and(depth)?),
            );
        }
        Ok(expression)
    }

    fn and(&mut self, depth: usize) -> Result<Expression> {
        let mut expression = self.unary(depth)?;
        loop {
            if matches!(
                self.tokens.get(self.position),
                None | Some(Token::Close)
            ) || self.tokens.get(self.position)
                == Some(&Token::Word("OR".into()))
            {
                break;
            }
            self.keyword("AND");
            expression = Expression::And(
                Box::new(expression),
                Box::new(self.unary(depth)?),
            );
        }
        Ok(expression)
    }

    fn unary(&mut self, depth: usize) -> Result<Expression> {
        ensure!(depth < 16, "Search nesting exceeds 16 levels");
        if self.keyword("NOT") {
            return Ok(Expression::Not(Box::new(self.unary(depth + 1)?)));
        }
        if self.tokens.get(self.position) == Some(&Token::Open) {
            self.position += 1;
            let expression = self.or(depth + 1)?;
            ensure!(
                self.tokens.get(self.position) == Some(&Token::Close),
                "Missing closing parenthesis"
            );
            self.position += 1;
            return Ok(expression);
        }
        self.term()
    }

    fn term(&mut self) -> Result<Expression> {
        let token = self
            .tokens
            .get(self.position)
            .context("Expected a search term")?;
        self.position += 1;
        let (field, value, phrase) = match token {
            Token::Phrase(value) => (String::new(), value.clone(), true),
            Token::Word(word) => {
                ensure!(
                    !["AND", "OR"].contains(&word.as_str()),
                    "Expected a term before {word}"
                );
                let (field, value) = word.split_once(':').unwrap_or(("", word));
                (field.to_owned(), value.to_owned(), false)
            }
            _ => bail!("Expected a search term"),
        };
        let (value, phrase) = if !field.is_empty() && value.is_empty() {
            let Some(Token::Phrase(quoted)) = self.tokens.get(self.position)
            else {
                bail!("Expected a value after {field}:");
            };
            self.position += 1;
            (quoted.clone(), true)
        } else {
            (value, phrase)
        };
        ensure!(!value.is_empty(), "Search term cannot be empty");
        Ok(Expression::Term {
            field,
            value,
            phrase,
        })
    }
}
