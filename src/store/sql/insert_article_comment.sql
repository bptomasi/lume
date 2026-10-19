INSERT INTO comments (
    id,
    article_id,
    body,
    quote,
    anchor,
    source,
    source_revision
)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);
