//! Separate SQL preparation, first-row execution and owned-value materialization.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, Row, StatementStatus, types::Value};

struct Query {
    name: &'static str,
    sql: String,
    parameters: Vec<i64>,
    expected: Vec<Value>,
}

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("profile.sqlite3");
    let notes = if stress { 100_000 } else { 10_000 };
    let store = super::annotation_notebook::seed(&path, notes, 100)?;
    let connection = Connection::open(&path)?;
    let queries = queries(&connection, notes as i64)?;
    println!("notebook SQL profile: SQLite {}", rusqlite::version());
    for query in queries {
        profile(&connection, &query, iterations * 100)?;
    }
    ensure!(store.search_notes("", None, 0, 1)?.total == notes);
    Ok(())
}

fn queries(connection: &Connection, note: i64) -> Result<Vec<Query>> {
    let (summary, article) = reference_queries(connection, note)?;
    let Value::Integer(owner) = summary.expected[1] else {
        anyhow::bail!("Note lost its article identity");
    };
    let body: String = connection.query_row(
        include_str!("../../src/store/sql/catalog/select_body.sql"),
        [owner],
        |row| row.get(0),
    )?;
    let url = article.expected[3].clone();
    Ok(vec![
        summary,
        article,
        Query {
            name: "export ownership and URL",
            sql: include_str!(
                "../../src/store/sql/notebook/select_export_source.sql"
            )
            .into(),
            parameters: vec![note],
            expected: vec![Value::Integer(owner), url.clone()],
        },
        source_query("Feed source", owner, true, url.clone(), body),
        source_query(
            "Full source without feed body",
            owner,
            false,
            url,
            String::new(),
        ),
    ])
}

fn reference_queries(
    connection: &Connection,
    note: i64,
) -> Result<(Query, Query)> {
    let summary_sql = format!(
        include_str!("../../src/store/sql/notebook/template_note_summary.sql"),
        include_str!("../../src/store/sql/notebook/select_note_page.sql")
    );
    let summary = connection.query_row(&summary_sql, [note], values)?;
    ensure!(summary[0] == Value::Integer(note));
    ensure!(summary[11] == Value::Text("[\"consensus\"]".into()));
    let Value::Integer(article) = summary[1] else {
        anyhow::bail!("Note lost its article identity");
    };
    let article_sql = format!(
        include_str!("../../src/store/sql/query/template_article.sql"),
        include_str!("../../src/store/sql/select_article_metadata.sql")
    );
    let metadata = connection.query_row(&article_sql, [article], values)?;
    ensure!(metadata[9] == Value::Integer(note / 100));
    Ok((
        Query {
            name: "original note summary",
            sql: summary_sql,
            parameters: vec![note],
            expected: summary,
        },
        Query {
            name: "original article metadata and note count",
            sql: article_sql,
            parameters: vec![article],
            expected: metadata,
        },
    ))
}

fn source_query(
    name: &'static str,
    article: i64,
    feed: bool,
    url: Value,
    body: String,
) -> Query {
    Query {
        name,
        sql: include_str!(
            "../../src/store/sql/notebook/select_source_context.sql"
        )
        .into(),
        parameters: vec![article, i64::from(feed)],
        expected: vec![url, Value::Text(body)],
    }
}

fn profile(connection: &Connection, query: &Query, count: usize) -> Result<()> {
    let mut elapsed = [Duration::ZERO; 5];
    let mut steps = None;
    for _ in 0..count {
        let started = Instant::now();
        let mut statement = connection.prepare(&query.sql)?;
        let prepared = Instant::now();
        let mut rows =
            statement.query(rusqlite::params_from_iter(&query.parameters))?;
        let row = rows.next()?.context("Profile query returned no row")?;
        let executed = Instant::now();
        let actual = values(row)?;
        let materialized = Instant::now();
        drop(rows);
        let reset = Instant::now();
        let operations = statement.get_status(StatementStatus::VmStep);
        ensure!(steps.is_none_or(|expected| expected == operations));
        steps = Some(operations);
        let finalizing = Instant::now();
        statement.finalize()?;
        let finalized = Instant::now();
        elapsed[0] += prepared - started;
        elapsed[1] += executed - prepared;
        elapsed[2] += materialized - executed;
        elapsed[3] += reset - materialized;
        elapsed[4] += finalized - finalizing;
        ensure!(actual == query.expected, "Query result changed");
    }
    print_profile(query.name, count, elapsed, steps.context("No samples")?);
    Ok(())
}

fn print_profile(name: &str, count: usize, elapsed: [Duration; 5], steps: i32) {
    print!("{name}: {count} checked iterations");
    for (label, duration) in [
        "prepare",
        "execute/bind",
        "materialize",
        "reset",
        "finalize",
    ]
    .into_iter()
    .zip(elapsed)
    {
        print!(
            ", {label}={:.3} us",
            duration.as_secs_f64() * 1e6 / count as f64
        );
    }
    println!(", VM steps={steps}");
}

fn values(row: &Row<'_>) -> rusqlite::Result<Vec<Value>> {
    (0..row.as_ref().column_count())
        .map(|column| row.get(column))
        .collect()
}
