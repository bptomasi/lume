CREATE TRIGGER reject_article_update
BEFORE UPDATE ON articles
BEGIN
    SELECT RAISE(ABORT, 'simulated disk/constraint failure');
END;
