-- Optional typesetting is appearance; source documents remain untouched.
CREATE TRIGGER configuration_graphic_formula_preferences_update
AFTER UPDATE OF value ON settings
WHEN NEW.key = 'workspace' AND json_valid(NEW.value)
    AND coalesce(json_extract(NEW.value, '$.latex_graphics'), 0)
        IS NOT coalesce(json_extract(
            CASE WHEN json_valid(OLD.value) THEN OLD.value ELSE '{}' END,
            '$.latex_graphics'
        ), 0)
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;

PRAGMA user_version = 16;
