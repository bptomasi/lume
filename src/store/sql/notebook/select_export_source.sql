SELECT c.article_id, a.url
FROM comments AS c
JOIN articles AS a ON a.id = c.article_id
JOIN feeds AS f ON f.id = a.feed_id
JOIN note_metadata AS m ON m.comment_id = c.id
WHERE c.id = ?1 AND a.deleted = 0;
