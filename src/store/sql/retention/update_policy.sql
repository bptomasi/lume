INSERT INTO settings(key, value) VALUES ('retention_policy', ?1)
ON CONFLICT(key) DO UPDATE SET value = excluded.value
WHERE value IS NOT excluded.value;
