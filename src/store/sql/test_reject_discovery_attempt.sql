CREATE TRIGGER reject_discovery_attempt BEFORE UPDATE OF attempted_at ON servers
WHEN NEW.attempted_at > 0 BEGIN
    SELECT raise(ABORT, 'Injected discovery health failure');
END;
