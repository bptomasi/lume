INSERT INTO collections (name)
VALUES (?1)
ON CONFLICT (name) DO UPDATE SET enabled = 1
WHERE collections.enabled = 0;
