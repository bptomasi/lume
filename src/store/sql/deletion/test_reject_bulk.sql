CREATE TRIGGER reject_bulk_deletion BEFORE UPDATE OF deleted ON articles
WHEN NEW.deleted = 1 AND NEW.id = 3
BEGIN
    SELECT RAISE(ABORT, 'Controlled bulk deletion failure');
END;
