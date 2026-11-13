SELECT url, CASE WHEN ?2 THEN body ELSE '' END
FROM articles
WHERE id = ?1;
