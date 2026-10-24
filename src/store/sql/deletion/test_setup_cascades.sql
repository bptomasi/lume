INSERT INTO servers(id, url, token_env) VALUES (10, 'https://reader.example', 'TOKEN');
INSERT INTO feeds(id, url, title, server_id) VALUES
    (1, 'https://reader.example/v1/feeds/7', 'Remote', 10),
    (2, 'https://publisher.example/rss', 'Survivor', NULL);
INSERT INTO server_feeds(server_id, remote_id, feed_id) VALUES (10, 7, 1);
INSERT INTO articles(id, feed_id, external_id, title, url, author, published, body, deleted)
VALUES (1, 1, 'miniflux:1', 'Purged', 'https://publisher.example/one', '', 1,
    'purgedfeed ![shared](https://images.example/shared) ![unique](https://images.example/unique)', 1),
    (2, 2, 'kept', 'Survivor', 'https://publisher.example/two', '', 2,
    'survivingfeed ![shared](https://images.example/shared)', 0);
INSERT INTO server_entries(server_id, remote_id, article_id) VALUES (10, 1, 1), (10, 2, 1);
INSERT INTO article_documents(article_id, body, url, final_url, attempt_url, attempted_at)
SELECT id, CASE id WHEN 1 THEN 'purgedfull' ELSE 'survivingfull' END,
    url, url, url, 1 FROM articles;
INSERT INTO comments(id, article_id, body) VALUES (1, 1, 'purgednote'), (2, 2, 'survivingnote');
INSERT INTO comment_tags VALUES (1, 'purged'), (2, 'kept');
INSERT INTO article_tags VALUES (1, 'purged'), (2, 'kept');
INSERT INTO collections(id, name) VALUES (1, 'Keep folder');
INSERT INTO members VALUES (1, 1), (1, 2);
INSERT INTO note_links VALUES (1, 2), (2, 1);
INSERT INTO note_references VALUES (1, 2), (2, 1);
INSERT INTO annotation_reviews(comment_id) VALUES (1), (2);
INSERT INTO study_cards(id, comment_id, question, answer) VALUES (1, 1, 'q', 'a'), (2, 2, 'q', 'a');
INSERT INTO export_manifest VALUES ('shared', 1, 'purged export'), ('shared', 2, 'kept export');
INSERT INTO note_export_manifest VALUES ('shared', 1, 'purged note export'), ('shared', 2, 'kept note export');
INSERT INTO recommendation_features VALUES (1, 1, 1, '{}', 0), (2, 1, 1, '{}', 0);
INSERT INTO recommendation_matches VALUES (1, 'rust', 1), (2, 'rust', 1);
INSERT INTO recommendation_items VALUES (1, 1), (2, 2);
INSERT INTO recommendation_feedback VALUES (1, 'useful'), (2, 'useful');
INSERT INTO recommendation_feedback_topics VALUES (1, 'rust'), (2, 'rust');
INSERT INTO note_drafts(key, target, body, baseline) VALUES
    ('first', '{"Note":{"article":1,"note":1}}', 'purged draft', ''),
    ('second', '{"Note":{"article":2,"note":2}}', 'kept draft', '');
