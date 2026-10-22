SELECT a.body, d.body
FROM articles AS a
LEFT JOIN article_documents AS d
    ON d.article_id = a.id AND d.url = a.url
WHERE a.url = ?1 AND a.deleted = 0
ORDER BY a.id
LIMIT 1;
