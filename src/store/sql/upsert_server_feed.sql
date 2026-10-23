-- A synthetic API identity must never take ownership of a local subscription.
INSERT INTO feeds (url, title, server_id, enabled)
VALUES (?1, ?2, ?3, 1);
