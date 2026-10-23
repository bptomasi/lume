SELECT a.title, f.title, a.unread,
    coalesce(c.remove_article, 0), coalesce(c.clear_full, 0), coalesce(c.expire_images, 0),
    coalesce(CASE WHEN a.unread THEN p.unread_cutoff ELSE p.read_cutoff END, 0),
    coalesce(p.full_cutoff, 0), coalesce(p.image_cutoff, 0)
FROM articles AS a JOIN feeds AS f ON f.id = a.feed_id
LEFT JOIN temp.retention_candidates AS c ON c.article_id = a.id
LEFT JOIN temp.retention_sources AS p ON p.feed_id = a.feed_id
WHERE a.id = ?1;
