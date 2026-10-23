INSERT INTO articles (
    id,
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
SELECT (SELECT last_id + 1 FROM article_identity_clock WHERE singleton = 1),
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, unixepoch(), ?10
WHERE NOT EXISTS (SELECT 1 FROM retention_tombstones
    WHERE feed_id = ?1 AND external_id = ?2)
    AND NOT (?10 <> '' AND EXISTS (SELECT 1 FROM retention_tombstones
        WHERE feed_id = ?1 AND canonical_key <> '' AND canonical_key = ?10))
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
    canonical_key = excluded.canonical_key
WHERE articles.title IS NOT excluded.title OR articles.url IS NOT excluded.url
    OR articles.author IS NOT excluded.author
    OR articles.published IS NOT excluded.published
    OR articles.body IS NOT excluded.body
    OR articles.body_origin IS NOT excluded.body_origin
    OR articles.canonical_key IS NOT excluded.canonical_key
    OR (articles.reading_minutes IS NOT excluded.reading_minutes
        AND NOT EXISTS (SELECT 1 FROM article_documents AS d
            WHERE d.article_id = articles.id AND d.url = excluded.url
                AND d.body <> ''));
