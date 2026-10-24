INSERT INTO export_manifest VALUES ('kept', 12, 'Exported source');
INSERT INTO comments(id, article_id, body) VALUES (13, 13, 'Historical note');
DELETE FROM comments WHERE id = 13;
INSERT INTO settings(key, value) VALUES ('reading_session',
    '{"budget":10,"position":1,"items":[
        {"article":15,"feed":1,"url":"https://journal.example/15","canonical":"","title":"Completed","minutes":1,"score":0,"topic":""},
        {"article":11,"feed":1,"url":"https://journal.example/11","canonical":"","title":"Remaining","minutes":1,"score":0,"topic":""}
    ]}');
