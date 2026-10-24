-- Deleted folders can still own identities in nested undo payloads.
DELETE FROM undo_log;
INSERT INTO undo_log (operation, payload) VALUES
    ('personal', '{"Batch":[{"DeleteCollection":80}]}'),
    ('personal', '{"Collection":{"id":90,"name":"Removed","articles":[]}}'),
    ('personal', '{"Collection":{"id":"invalid"}}'),
    ('personal', 'invalid legacy JSON');
