UPDATE feeds SET url = ?2, title = ?3
WHERE id = ?1 AND (url IS NOT ?2 OR title IS NOT ?3);
