UPDATE comments
SET body = ?1
WHERE id = ?2
    AND article_id = ?3;
