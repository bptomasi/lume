UPDATE comments
SET body = ?1,
    quote = ?2,
    anchor = ?3,
    source = ?4,
    source_revision = ?5
WHERE id = ?6 AND article_id = ?7;
