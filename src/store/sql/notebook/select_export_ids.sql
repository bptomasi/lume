SELECT c.id
FROM comments AS c
JOIN articles AS a ON a.id = c.article_id
JOIN feeds AS f ON f.id = a.feed_id
JOIN note_metadata AS m ON m.comment_id = c.id
WHERE a.deleted = 0
ORDER BY m.pinned DESC, m.updated_at DESC, c.id DESC
LIMIT ?1 OFFSET ?2;
