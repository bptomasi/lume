INSERT INTO note_export_manifest (namespace, note_id, content)
VALUES (?1, ?2, ?3)
ON CONFLICT (namespace, note_id) DO UPDATE SET
    content = excluded.content;
