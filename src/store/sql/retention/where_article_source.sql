WHERE f.id = (SELECT feed_id FROM articles WHERE id = ?1)
