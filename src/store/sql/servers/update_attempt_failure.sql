UPDATE servers
SET attempted_at = ?1, error = ?2
WHERE id = ?3;
