INSERT INTO feeds (url, title, server_id, enabled)
VALUES (?1, ?2, ?3, 1)
ON CONFLICT (url) DO UPDATE SET
    title = excluded.title,
    server_id = excluded.server_id
WHERE feeds.title <> excluded.title
    OR feeds.server_id IS NOT excluded.server_id;
