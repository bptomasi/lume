INSERT INTO article_documents (
    article_id,
    url,
    final_url,
    body,
    revision,
    fetched_at,
    attempt_url,
    attempted_at,
    error
)
VALUES (?1, ?2, ?3, ?4, 1, unixepoch(), ?2, unixepoch(), '')
ON CONFLICT (article_id) DO UPDATE SET
    revision = article_documents.revision + CASE
        WHEN article_documents.body != excluded.body
            OR article_documents.url != excluded.url THEN 1
        ELSE 0
    END,
    progress = CASE
        WHEN article_documents.body = excluded.body
            AND article_documents.url = excluded.url
            THEN article_documents.progress
        ELSE 0
    END,
    url = excluded.url,
    final_url = excluded.final_url,
    body = excluded.body,
    fetched_at = excluded.fetched_at,
    attempt_url = excluded.attempt_url,
    attempted_at = excluded.attempted_at,
    error = '';
