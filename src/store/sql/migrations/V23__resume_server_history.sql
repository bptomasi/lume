ALTER TABLE servers ADD COLUMN sync_settings TEXT NOT NULL DEFAULT '{"initial_entries":2000,"entries_per_page":200,"pages_per_sync":10}' CHECK(json_valid(sync_settings));
ALTER TABLE server_feeds ADD COLUMN history_before_id INTEGER NOT NULL DEFAULT 0;
ALTER TABLE server_feeds ADD COLUMN history_loaded INTEGER NOT NULL DEFAULT 0;
ALTER TABLE server_feeds ADD COLUMN history_total INTEGER NOT NULL DEFAULT 0;
CREATE TRIGGER configuration_server_sync_update AFTER UPDATE OF sync_settings ON servers
WHEN NEW.sync_settings IS NOT OLD.sync_settings BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;
PRAGMA user_version = 23;
