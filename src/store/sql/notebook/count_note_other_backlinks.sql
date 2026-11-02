SELECT count(*)
FROM note_references
WHERE target_id = ?1 AND source_id <> ?2;
