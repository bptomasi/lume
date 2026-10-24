DELETE FROM note_export_manifest
WHERE note_id IN (SELECT id FROM temp.purge_notes);
DELETE FROM note_versions
WHERE article_id IN (SELECT id FROM temp.purge_articles);
DELETE FROM note_drafts
WHERE coalesce(json_extract(target, '$.Note.article'),
               json_extract(target, '$.Passage.article'),
               json_extract(target, '$.Card.article'))
    IN (SELECT id FROM temp.purge_articles);
-- Foreign keys remove notes, memberships, mappings, references and documents;
-- their owning triggers remove both article and note full-text search entries.
DELETE FROM articles WHERE id IN (SELECT id FROM temp.purge_articles);
