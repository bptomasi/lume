SELECT a.body, coalesce(d.body, ''),
    CASE WHEN c.expire_images
        AND NOT EXISTS (SELECT 1 FROM protected WHERE article_id = a.id)
        AND NOT EXISTS (SELECT 1 FROM temp.retention_smart WHERE article_id = a.id)
    THEN c.image_cutoff ELSE 0 END
FROM articles AS a
LEFT JOIN article_documents AS d ON d.article_id = a.id
LEFT JOIN temp.retention_candidates AS c ON c.article_id = a.id;
