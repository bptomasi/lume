INSERT INTO articles (
    feed_id,
    external_id,
    title,
    url,
    author,
    published,
    body,
    body_origin,
    reading_minutes,
    added_at,
    canonical_key
)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, unixepoch(), ?10)
ON CONFLICT (feed_id, external_id) DO UPDATE SET
    title = excluded.title,
    url = excluded.url,
    author = excluded.author,
    published = excluded.published,
    body = excluded.body,
    body_origin = excluded.body_origin,
    reading_minutes = CASE WHEN EXISTS (
        SELECT 1 FROM article_documents AS d
        WHERE d.article_id = articles.id AND d.url = excluded.url AND d.body <> ''
    ) THEN articles.reading_minutes ELSE excluded.reading_minutes END,
    canonical_key = excluded.canonical_key;
