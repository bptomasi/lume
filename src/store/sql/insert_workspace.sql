INSERT INTO settings (key, value) VALUES ('workspace', ?1)
ON CONFLICT (key) DO UPDATE SET value = excluded.value
WHERE settings.value IS NOT excluded.value;
