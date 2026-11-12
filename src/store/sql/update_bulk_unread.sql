UPDATE articles SET unread = ?1
WHERE id IN (SELECT value FROM json_each(?2))
    AND deleted = 0 AND unread IS NOT ?1;
