UPDATE articles
SET title = ?2,
    url = ?3,
    author = ?4,
    published = ?5,
    body = ?6,
    body_origin = ?7,
    reading_minutes = CASE WHEN EXISTS (
        SELECT 1 FROM article_documents AS d
        WHERE d.article_id = articles.id
            AND d.url = ?3
            AND d.body <> ''
    ) THEN articles.reading_minutes ELSE ?8 END,
    canonical_key = ?9
WHERE id = ?1;
