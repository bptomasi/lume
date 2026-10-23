ALTER TABLE servers ADD COLUMN token TEXT NOT NULL DEFAULT '';

CREATE TRIGGER configuration_server_token_update AFTER UPDATE OF token ON servers
WHEN NEW.token IS NOT OLD.token
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;

CREATE TRIGGER server_credentials_recovered
AFTER UPDATE OF url, token, token_env, enabled ON servers
WHEN NEW.enabled = 1 AND (
    NEW.url IS NOT OLD.url OR NEW.token IS NOT OLD.token
    OR NEW.token_env IS NOT OLD.token_env OR NEW.enabled IS NOT OLD.enabled
)
BEGIN
    UPDATE feeds SET error = '', retry_at = 0, failures = 0
    WHERE server_id = NEW.id AND (error <> '' OR retry_at <> 0 OR failures <> 0);
    UPDATE servers SET error = '' WHERE id = NEW.id AND error <> '';
END;

PRAGMA user_version = 20;
