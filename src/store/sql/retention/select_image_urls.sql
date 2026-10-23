SELECT r.url, min(
    CASE WHEN c.expire_images
        AND NOT EXISTS (SELECT 1 FROM protected WHERE article_id = a.id)
        AND NOT EXISTS (SELECT 1 FROM temp.retention_smart WHERE article_id = a.id)
    THEN c.image_cutoff ELSE 0 END)
FROM image_references AS r
JOIN articles AS a ON a.id = r.article_id
LEFT JOIN temp.retention_candidates AS c ON c.article_id = a.id
WHERE r.cache_bucket IN (SELECT value FROM json_each(?1))
GROUP BY r.url;
