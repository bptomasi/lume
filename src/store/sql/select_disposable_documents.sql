SELECT d.article_id, length(CAST(d.body AS BLOB)), a.body
FROM article_documents AS d
JOIN articles AS a ON a.id = d.article_id
WHERE d.article_id > ?1 AND d.article_id <= ?2 AND d.body <> ''
    AND NOT EXISTS (SELECT 1 FROM protected WHERE article_id = a.id)
ORDER BY d.article_id
LIMIT 16;
