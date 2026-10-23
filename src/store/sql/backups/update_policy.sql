INSERT INTO settings(key, value) VALUES ('backup_policy', ?1)
ON CONFLICT(key) DO UPDATE SET value = excluded.value;
