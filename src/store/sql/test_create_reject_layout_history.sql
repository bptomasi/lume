CREATE TRIGGER reject_layout_history
BEFORE INSERT ON settings
WHEN NEW.key = 'layouts'
BEGIN
    SELECT RAISE(ABORT, 'Injected layout journal failure');
END;
