SELECT id, deleted, unread, starred, queued
FROM articles
WHERE id IN (SELECT value FROM json_each(?1))
ORDER BY id;
