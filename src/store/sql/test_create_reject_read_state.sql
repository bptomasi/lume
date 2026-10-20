CREATE TRIGGER reject_read_state
BEFORE UPDATE OF unread ON articles
BEGIN
    SELECT RAISE(ABORT, 'read-state fixture rejects writes');
END;
