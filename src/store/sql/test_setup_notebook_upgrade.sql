INSERT INTO comment_tags (comment_id, tag) VALUES (1, 'consensus');
INSERT INTO note_links (comment_id, article_id) VALUES (1, 2);
INSERT INTO annotation_reviews (comment_id, due_at, interval_days, reviews)
VALUES (1, 604900, 7, 3);
INSERT INTO study_cards (
    id, comment_id, question, answer, due_at, interval_days, reviews
)
VALUES (9, 1, 'Who owns the transaction?', 'The repository', 259300, 3, 2);
