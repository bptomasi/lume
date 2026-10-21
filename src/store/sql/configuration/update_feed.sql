UPDATE feeds
SET url = ?2,
    enabled = ?3,
    article_selector = ?4,
    etag = CASE WHEN url <> ?2 THEN '' ELSE etag END,
    modified = CASE WHEN url <> ?2 THEN '' ELSE modified END
WHERE id = ?1
    AND (url <> ?2 OR enabled <> ?3 OR article_selector <> ?4);
