UPDATE article_documents
SET
    body = '',
    revision = revision + 1,
    progress = 0,
    fetched_at = 0,
    error = '',
    attempted_at = 0
WHERE article_id = ?1;
