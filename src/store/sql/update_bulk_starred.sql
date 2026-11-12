UPDATE articles SET starred = ?1
WHERE id IN (SELECT value FROM json_each(?2))
    AND deleted = 0 AND starred IS NOT ?1;
