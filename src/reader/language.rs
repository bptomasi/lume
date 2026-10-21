//! Bounded language hints select reviewed grammars; they never parse code.
//!
//! Explicit publisher choices take precedence. Only strong SQL/shell evidence
//! supplies a missing hint; ambiguous or explicitly unknown blocks stay plain.

const SQL_PREFIXES: &[&str] = &[
    "CREATE TABLE ",
    "INSERT INTO ",
    "SELECT ",
    "ALTER TABLE ",
    "CALL ",
];

const SHELL_COMMANDS: &[&str] = &[
    "kubectl ",
    "export ",
    "curl ",
    "docker ",
    "helm ",
    "terraform ",
    "git ",
];

pub(super) fn resolve(text: &str, info: &str) -> String {
    let token = info
        .split(|c: char| c.is_whitespace() || c == ',')
        .next()
        .unwrap_or_default()
        .trim_matches(|c| matches!(c, '{' | '}' | '.'))
        .to_ascii_lowercase();
    let token = token
        .strip_prefix("language-")
        .or_else(|| token.strip_prefix("lang-"))
        .unwrap_or(&token);
    let canonical = match token {
        "golang" => "go",
        "node" | "nodejs" | "ecmascript" => "javascript",
        "shell" | "shell-session" | "console" => "bash",
        "tf" | "tfvars" | "hcl" => "terraform",
        "duckdb" | "postgres" | "postgresql" | "mysql" | "sql-session" => "sql",
        "" => infer(text),
        _ => token,
    };
    canonical.to_owned()
}

fn infer(text: &str) -> &'static str {
    let bounded = &text[..text.floor_char_boundary(text.len().min(16_384))];
    let lines = bounded
        .lines()
        .take(128)
        .map(str::trim_start)
        .collect::<Vec<_>>();
    let sql = SQL_PREFIXES
        .iter()
        .filter(|prefix| {
            lines.iter().any(|line| {
                line.get(..prefix.len())
                    .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
            })
        })
        .count();
    if sql >= 2 && bounded.contains(';') {
        return "sql";
    }
    let shell = lines
        .iter()
        .filter(|line| {
            let line = line.strip_prefix("$ ").unwrap_or(line);
            SHELL_COMMANDS
                .iter()
                .any(|command| line.starts_with(command))
        })
        .count();
    if shell >= 2
        || bounded.starts_with("#!/bin/")
        || bounded.starts_with("#!/usr/bin/env ")
    {
        return "bash";
    }
    ""
}
