INSERT INTO comments(id, article_id, body) SELECT id, id, 'A saved annotation' FROM articles;
INSERT INTO study_cards(comment_id, question, answer) SELECT id, 'Question', 'Answer' FROM comments;
INSERT INTO export_manifest(namespace, article_id, content) SELECT 'bench', id, 'Exported article' FROM articles;
INSERT INTO note_export_manifest(namespace, note_id, content) SELECT 'bench', id, 'Exported note' FROM comments;
