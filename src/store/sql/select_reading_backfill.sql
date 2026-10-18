SELECT
    a.id,
    CASE WHEN d.url = a.url AND d.body <> '' THEN d.body ELSE a.body END,
    a.url
FROM articles AS a
LEFT JOIN article_documents AS d ON d.article_id = a.id
WHERE a.id > ?1
ORDER BY a.id
LIMIT 32;
