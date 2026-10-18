SELECT c.article_id, c.id
FROM comments AS c
JOIN articles AS a ON a.id = c.article_id
LEFT JOIN annotation_reviews AS r ON r.comment_id = c.id
WHERE a.deleted = 0
    AND c.quote <> ''
    AND coalesce(r.due_at, 0) <= ?1
ORDER BY coalesce(r.due_at, 0), c.id
LIMIT 200;
