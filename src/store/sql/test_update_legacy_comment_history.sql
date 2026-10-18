UPDATE undo_log
SET payload = json_remove(
    payload,
    '$.Comment.comment.source',
    '$.Comment.comment.source_revision'
)
WHERE id = (SELECT max(id) FROM undo_log);
