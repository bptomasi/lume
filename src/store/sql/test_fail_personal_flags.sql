CREATE TRIGGER fail_personal_flags
BEFORE UPDATE OF unread, starred, queued ON articles
WHEN NEW.id = 2
BEGIN
    SELECT RAISE(ABORT, 'Personal flag write failed');
END;
