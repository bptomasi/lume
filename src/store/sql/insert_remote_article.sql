INSERT INTO articles (
    feed_id,
    external_id,
    title,
    url,
    author,
    published,
    body,
    body_origin,
    reading_minutes,
    unread,
    starred,
    added_at,
    canonical_key
)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, unixepoch(), ?12);
