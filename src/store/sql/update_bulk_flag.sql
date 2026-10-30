UPDATE articles SET {column} = ?1
WHERE id IN (SELECT value FROM json_each(?2))
    AND deleted = 0 AND {column} IS NOT ?1;
