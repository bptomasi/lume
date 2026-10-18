INSERT INTO article_documents (
    article_id,
    attempt_url,
    attempted_at,
    error
)
VALUES (?1, ?2, unixepoch(), ?3)
ON CONFLICT (article_id) DO UPDATE SET
    attempt_url = excluded.attempt_url,
    attempted_at = excluded.attempted_at,
    error = excluded.error;
