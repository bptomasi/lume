UPDATE articles SET starred = 1 WHERE id = 1;
UPDATE articles SET queued = 1 WHERE id = 2;
INSERT INTO collections(id, name) VALUES (1, 'Personal archive');
INSERT INTO members(collection_id, article_id) VALUES (1, 3);
INSERT INTO comments(id, article_id, body, quote) VALUES (1, 4, '', 'Highlight');
INSERT INTO comments(id, article_id, body) VALUES (2, 5, 'Private note');
INSERT INTO note_links(comment_id, article_id) VALUES (2, 6);
INSERT INTO article_tags(article_id, tag) VALUES (7, 'keep');
INSERT INTO note_drafts(key, target, body, baseline)
VALUES ('note:8:None', '{"Note":{"article":8,"note":null}}', 'Unsaved thought', '');
INSERT INTO undo_log(operation, payload)
VALUES ('personal', '{"Flags":{"article":9,"unread":false,"starred":false,"queued":false}}');
