UPDATE note_metadata
SET kind = ?1,
    status = ?2,
    pinned = ?3,
    updated_at = ?4
WHERE comment_id = ?5;
