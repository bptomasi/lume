CREATE TRIGGER reject_permanent_deletion BEFORE DELETE ON articles
BEGIN
    SELECT RAISE(ABORT, 'Controlled permanent deletion failure');
END;
