CREATE TRIGGER test_keep_article_tag BEFORE DELETE ON article_tags
WHEN OLD.tag = 'keep'
BEGIN SELECT RAISE(ABORT, 'unchanged tag was removed'); END;
