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
FROM articles AS a
JOIN feeds AS f ON f.id = a.feed_id
WHERE a.deleted = 0
ORDER BY a.published DESC, a.id DESC
LIMIT 10000;
