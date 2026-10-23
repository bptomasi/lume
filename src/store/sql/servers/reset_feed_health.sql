UPDATE feeds SET error = '', retry_at = 0, failures = 0
WHERE server_id = ?1 AND (error <> '' OR retry_at <> 0 OR failures <> 0);
