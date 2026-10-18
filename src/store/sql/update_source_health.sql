UPDATE feeds
SET
    attempted_at = ?2,
    succeeded_at = CASE WHEN ?3 = '' THEN ?2 ELSE succeeded_at END,
    retry_at = CASE WHEN ?3 = '' THEN 0 ELSE ?4 END,
    failures = CASE WHEN ?3 = '' THEN 0 ELSE min(failures + 1, 30) END,
    error = ?3
WHERE id = ?1;
