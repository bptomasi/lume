INSERT INTO settings (key, value)
VALUES ('layouts', ?1)
ON CONFLICT (key) DO UPDATE
SET value = excluded.value
