INSERT INTO collections (name)
VALUES ('Saved pages')
ON CONFLICT (name) DO UPDATE SET enabled = 1;
