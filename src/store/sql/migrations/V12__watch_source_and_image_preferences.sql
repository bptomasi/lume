-- Publish source policy and image settings without treating progress as config.
CREATE TRIGGER configuration_feed_policy_update
AFTER UPDATE OF article_policy ON feeds
WHEN NEW.article_policy IS NOT OLD.article_policy
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;

CREATE TRIGGER configuration_image_preferences_update
AFTER UPDATE OF value ON settings
WHEN NEW.key = 'workspace' AND json_valid(NEW.value)
    AND json_extract(NEW.value, '$.images') IS NOT json_extract(
        CASE WHEN json_valid(OLD.value) THEN OLD.value ELSE '{}' END,
        '$.images'
    )
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;

PRAGMA user_version = 12;
