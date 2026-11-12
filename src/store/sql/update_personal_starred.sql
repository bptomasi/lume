UPDATE articles SET starred = ?1 WHERE id = ?2 AND starred IS NOT ?1;
