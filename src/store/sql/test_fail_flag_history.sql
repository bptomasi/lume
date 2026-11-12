CREATE TRIGGER fail_flag_history
BEFORE INSERT ON undo_log
BEGIN
    SELECT RAISE(ABORT, 'Flag history write failed');
END;
