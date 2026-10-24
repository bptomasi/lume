-- Reserve issued identities independently of physical definition deletion.
-- Old folder inverses can still own IDs above every live folder.
CREATE TABLE definition_identity_clock (
    kind TEXT PRIMARY KEY CHECK(kind IN ('folder', 'smart', 'notes')),
    last_id INTEGER NOT NULL CHECK(last_id >= 0)
);
INSERT INTO definition_identity_clock VALUES
    ('folder', coalesce((SELECT max(id) FROM collections), 0)),
    ('smart', coalesce((SELECT max(id) FROM smart_lists), 0)),
    ('notes', coalesce((SELECT max(id) FROM note_collections), 0));
UPDATE definition_identity_clock SET last_id = max(last_id, coalesce((
    SELECT max(CASE WHEN field.key = 'Collection'
        THEN json_extract(field.value, '$.id') ELSE field.value END)
    FROM undo_log AS undo,
        json_tree(CASE WHEN json_valid(undo.payload) THEN undo.payload END) AS field
    WHERE (field.key = 'Collection' AND field.type = 'object'
        AND json_type(field.value, '$.id') = 'integer')
        OR (field.key = 'DeleteCollection' AND field.type = 'integer')
), 0)) WHERE kind = 'folder';
CREATE TRIGGER collection_identity_insert AFTER INSERT ON collections BEGIN
    UPDATE definition_identity_clock SET last_id = max(last_id, NEW.id)
    WHERE kind = 'folder';
END;
CREATE TRIGGER smart_identity_insert AFTER INSERT ON smart_lists BEGIN
    UPDATE definition_identity_clock SET last_id = max(last_id, NEW.id)
    WHERE kind = 'smart';
END;
CREATE TRIGGER note_collection_identity_insert AFTER INSERT ON note_collections BEGIN
    UPDATE definition_identity_clock SET last_id = max(last_id, NEW.id)
    WHERE kind = 'notes';
END;
-- This revision also owns the optional paused smart-filter field.
PRAGMA user_version = 27;
