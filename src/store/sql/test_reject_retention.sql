CREATE TRIGGER reject_retention BEFORE DELETE ON articles
WHEN OLD.id = 2
BEGIN
    SELECT RAISE(ABORT, 'Controlled retention failure');
END;
