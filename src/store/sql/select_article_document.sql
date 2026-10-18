SELECT
    url,
    final_url,
    body,
    revision,
    fetched_at,
    attempt_url,
    attempted_at,
    error,
    progress
FROM article_documents
WHERE article_id = ?1;
