UPDATE servers
SET attempted_at = ?1, succeeded_at = ?1, error = ''
WHERE id = ?2;
