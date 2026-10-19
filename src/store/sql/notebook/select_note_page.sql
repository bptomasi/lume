SELECT
    c.id,
    c.article_id,
    a.title,
    f.title,
    substr(c.body, 1, 512),
    substr(c.quote, 1, 512),
    c.created,
    m.kind,
    m.status,
    m.pinned,
    m.updated_at,
    (
        SELECT json_group_array(tag)
        FROM (
            SELECT tag
            FROM comment_tags
            WHERE comment_id = c.id
            ORDER BY tag COLLATE NOCASE
        )
    )
FROM comments AS c
JOIN articles AS a ON a.id = c.article_id
JOIN feeds AS f ON f.id = a.feed_id
JOIN note_metadata AS m ON m.comment_id = c.id
