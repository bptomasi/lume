//! Streaming Markdown article/notebook exports with source context.

use std::{io::Write, path::Path};

use anyhow::Result;

use crate::{
    model::{ArticleQuery, View},
    store::Store,
};

/// Export one cached article and its annotations to a new Markdown file.
///
/// # Errors
/// Returns missing article, SQLite or filesystem errors.
pub fn article(store: &Store, id: i64, path: &Path) -> Result<()> {
    super::atomic_file(path, |writer| write_article(store, id, writer, true))
}

/// Export all annotated articles, paging metadata and streaming each article.
///
/// # Errors
/// Returns SQLite or filesystem errors; incomplete files are never published.
pub fn notebook(store: &Store, path: &Path) -> Result<()> {
    super::atomic_file(path, |writer| {
        writeln!(writer, "# Lume · Reading notebook\n")?;
        let mut offset = 0;
        loop {
            let page = store.query_articles(&ArticleQuery {
                view: View::Annotated,
                text: String::new(),
                offset,
                limit: 200,
                retained: None,
            })?;
            if page.articles.is_empty() {
                break;
            }
            offset += page.articles.len();
            for article in page.articles {
                write_article(store, article.id, writer, false)?;
                writeln!(writer, "\n---\n")?;
            }
        }
        Ok(())
    })
}

pub(super) fn write_article(
    store: &Store,
    id: i64,
    writer: &mut dyn Write,
    include_body: bool,
) -> Result<()> {
    let article = store.article(id)?;
    writeln!(writer, "# {}\n", crate::content::label(&article.title, 500))?;
    writeln!(writer, "Source: {}\n\n<{}>\n", article.feed, article.url)?;
    if !article.author.is_empty() {
        writeln!(writer, "Author: {}\n", article.author)?;
    }
    if include_body {
        write_bodies(store, &article, writer)?;
    }
    writeln!(writer, "## Private annotations\n")?;
    for comment in store.comments(id)? {
        write_annotation(store, id, &comment, writer)?;
    }
    Ok(())
}

fn write_bodies(
    store: &Store,
    article: &crate::model::Article,
    writer: &mut dyn Write,
) -> Result<()> {
    if let Some(cache) = store.cached_document(article.id)?
        && cache.url == article.url
        && !cache.body.is_empty()
    {
        writeln!(
            writer,
            "Content: Full article, revision {}\n",
            cache.revision
        )?;
        writeln!(writer, "Retrieved from: <{}>\n", cache.final_url)?;
        writeln!(writer, "{}\n\n## Feed content\n", cache.body)?;
    }
    writeln!(writer, "{}\n", store.body(article.id)?)?;
    Ok(())
}

fn write_annotation(
    store: &Store,
    id: i64,
    comment: &crate::model::Comment,
    writer: &mut dyn Write,
) -> Result<()> {
    let context = store.annotation(id, comment.id)?;
    if !context.tags.is_empty() {
        writeln!(
            writer,
            "Tags: {}\n",
            context
                .tags
                .iter()
                .map(|tag| format!("#{tag}"))
                .collect::<Vec<_>>()
                .join(" ")
        )?;
    }
    for related in &context.links {
        writeln!(writer, "Related: [[Lume-{related}]]\n")?;
    }
    if !comment.quote.is_empty() {
        if comment.source == crate::model::ContentSource::Full {
            writeln!(
                writer,
                "Quote source: Full article, revision {}\n",
                comment.source_revision
            )?;
        }
        for line in comment.quote.lines() {
            writeln!(writer, "> {line}")?;
        }
        writeln!(writer)?;
    }
    writeln!(writer, "{}\n", crate::content::clean(&comment.body))?;
    for card in &context.cards {
        writeln!(
            writer,
            "Question: {}\n\nAnswer: {}\n",
            card.question, card.answer
        )?;
    }
    Ok(())
}
