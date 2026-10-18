CREATE TRIGGER test_reject_history
BEFORE INSERT ON undo_log
BEGIN
    SELECT RAISE(ABORT, 'Injected history write failure');
END;
