-- Remote definitions publish like local subscriptions. Cached content does not.
CREATE TRIGGER configuration_servers_insert AFTER INSERT ON servers
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;

CREATE TRIGGER configuration_servers_update AFTER UPDATE ON servers
WHEN NEW.url IS NOT OLD.url OR NEW.token_env IS NOT OLD.token_env
    OR NEW.enabled IS NOT OLD.enabled
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;

CREATE TRIGGER configuration_server_binding_update
AFTER UPDATE OF present ON configuration_bindings
WHEN NEW.kind = 'server' AND NEW.present IS NOT OLD.present
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;

-- Retain a bounded page cursor when many entries share one change timestamp.
ALTER TABLE server_feeds ADD COLUMN sync_offset INTEGER NOT NULL DEFAULT 0;

ALTER TABLE server_feeds ADD COLUMN sync_latest INTEGER NOT NULL DEFAULT 0;

PRAGMA user_version = 18;
