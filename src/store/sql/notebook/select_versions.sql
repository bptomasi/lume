SELECT id, article_id, note_id, body, quote, anchor, source,
       source_revision, saved_at
FROM note_versions
WHERE note_id = ?1
ORDER BY id DESC
LIMIT 100;
