UPDATE articles SET reading_minutes = ?1
WHERE id = ?2 AND reading_minutes IS NOT ?1;
