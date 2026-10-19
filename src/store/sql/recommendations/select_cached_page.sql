SELECT
    a.id,
    a.feed_id,
    a.title,
    a.url,
    f.title,
    a.author,
    a.published,
    a.unread,
    a.starred,
    (SELECT count(*) FROM comments AS c WHERE c.article_id = a.id),
    a.queued
FROM matching AS selected
CROSS JOIN articles AS a ON a.id = selected.id
JOIN feeds AS f ON f.id = a.feed_id
ORDER BY selected.position
LIMIT :limit OFFSET :offset;
