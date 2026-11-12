UPDATE articles SET queued = ?1 WHERE id = ?2 AND queued IS NOT ?1;
