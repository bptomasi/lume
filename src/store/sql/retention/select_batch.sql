SELECT a.id, CASE WHEN NOT ?3 THEN a.body ELSE '' END,
    coalesce(length(CAST(d.body AS BLOB)), 0),
    CASE WHEN ?3 THEN length(CAST(a.body AS BLOB)) ELSE 0 END
FROM temp.retention_candidates AS c
JOIN articles AS a ON a.id = c.article_id
LEFT JOIN article_documents AS d ON d.article_id = a.id
WHERE a.id > ?1 AND a.id <= ?2
    AND CASE WHEN ?3 THEN c.remove_article ELSE c.clear_full AND NOT c.remove_article END
    AND NOT EXISTS (SELECT 1 FROM protected WHERE article_id = a.id)
    AND NOT EXISTS (SELECT 1 FROM temp.retention_smart WHERE article_id = a.id)
ORDER BY a.id LIMIT ?4;
