SELECT kind, status, pinned, updated_at
FROM note_metadata
WHERE comment_id = ?1;
