SELECT CASE WHEN d.url = ?2 AND octet_length(d.body) > 0
    THEN octet_length(d.body) ELSE octet_length(a.body) END
FROM articles AS a LEFT JOIN article_documents AS d ON d.article_id = a.id
WHERE a.id = ?1 AND a.url = ?2 AND a.deleted = 0;
