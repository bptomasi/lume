SELECT coalesce(sum(c.remove_article), 0),
    coalesce(sum(c.clear_full AND NOT c.remove_article), 0),
    coalesce(sum(CASE WHEN c.remove_article OR c.clear_full
        THEN length(CAST(coalesce(d.body, '') AS BLOB)) ELSE 0 END), 0),
    coalesce(sum(CASE WHEN c.remove_article
        THEN length(CAST(a.body AS BLOB)) ELSE 0 END), 0)
FROM temp.retention_candidates AS c
JOIN articles AS a ON a.id = c.article_id
LEFT JOIN article_documents AS d ON d.article_id = a.id;
