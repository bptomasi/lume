UPDATE note_metadata
SET kind = 'definition'
WHERE comment_id % 10 = 0;

INSERT INTO comment_tags (comment_id, tag)
SELECT id, 'consensus'
FROM comments
WHERE id % 10 = 0;
