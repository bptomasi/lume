UPDATE articles
SET queued = 1, progress = 7
WHERE id = 42;

INSERT INTO settings (key, value)
VALUES ('workspace', '{"view":"All","article":42}');

INSERT INTO undo_log (operation, payload)
VALUES (
    'personal',
    '{
        "Flags": {
            "article": 42,
            "unread": true,
            "starred": false,
            "queued": false
        }
    }'
);
