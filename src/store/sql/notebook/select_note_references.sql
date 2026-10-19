SELECT source_id, target_id
FROM note_references
WHERE source_id = ?1 OR target_id = ?1
ORDER BY source_id, target_id;
