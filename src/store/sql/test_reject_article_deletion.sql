CREATE TRIGGER reject_article_deletion BEFORE UPDATE OF deleted ON articles
WHEN NEW.deleted != OLD.deleted
BEGIN
    SELECT RAISE(ABORT, 'Controlled article deletion failure');
END;
