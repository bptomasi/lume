CREATE TRIGGER reject_collected_attempt BEFORE UPDATE OF attempted_at ON feeds
WHEN NEW.attempted_at > 0 BEGIN
    SELECT raise(ABORT, 'Injected health update failure');
END;
