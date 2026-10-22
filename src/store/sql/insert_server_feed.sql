INSERT INTO server_feeds (
    server_id,
    remote_id,
    remote_url,
    feed_id,
    present
)
VALUES (?1, ?2, ?3, ?4, 1)
ON CONFLICT (server_id, remote_id) DO UPDATE SET
    remote_url = excluded.remote_url,
    present = 1;
