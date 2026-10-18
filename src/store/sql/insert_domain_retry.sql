INSERT INTO settings (key, value)
VALUES (?1, ?2)
ON CONFLICT (key) DO UPDATE SET
    value = excluded.value;
