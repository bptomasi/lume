INSERT INTO settings(key, value) VALUES ('backup_last_run', ?1)
ON CONFLICT(key) DO UPDATE SET value = excluded.value;
