INSERT INTO server_entries (
    server_id, remote_id, article_id
) VALUES (?1, ?2, ?3)
ON CONFLICT (server_id, remote_id) DO NOTHING;
