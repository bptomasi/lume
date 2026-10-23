INSERT INTO servers (id, url, token_env)
VALUES (77, 'https://reader.example', 'LUME_TOKEN');
UPDATE feeds SET server_id = 77 WHERE id = 1;
INSERT INTO server_feeds (
    server_id, remote_id, feed_id, initialized, last_changed, sync_offset,
    sync_latest
) VALUES (77, 7, 1, 1, 100, 2000, 500);
INSERT INTO server_entries (server_id, remote_id, article_id)
VALUES (77, 88, 1);
