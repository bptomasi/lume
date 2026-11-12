UPDATE articles SET queued = ?1
WHERE id IN (SELECT value FROM json_each(?2))
    AND deleted = 0 AND queued IS NOT ?1;
