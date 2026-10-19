SELECT
    a.id,
    a.title,
    a.reading_minutes,
    a.starred,
    a.body_origin,
    a.body,
    CASE WHEN d.url = a.url THEN d.body ELSE '' END,
    a.feed_id
FROM articles AS a
LEFT JOIN article_documents AS d ON d.article_id = a.id
WHERE a.id IN (SELECT value FROM json_each(?1));
