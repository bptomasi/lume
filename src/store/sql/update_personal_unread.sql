UPDATE articles SET unread = ?1 WHERE id = ?2 AND unread IS NOT ?1;
