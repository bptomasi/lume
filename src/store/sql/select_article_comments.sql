SELECT id, body, created, quote, anchor, source, source_revision
FROM comments
WHERE article_id = ?1
ORDER BY id;
