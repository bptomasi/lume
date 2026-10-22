-- A formula-only appearance edit must publish back to declarative TOML.
CREATE TRIGGER configuration_formula_preferences_update
AFTER UPDATE OF value ON settings
WHEN NEW.key = 'workspace' AND json_valid(NEW.value)
    AND coalesce(json_extract(NEW.value, '$.latex_formulas'), 1)
        IS NOT coalesce(json_extract(
            CASE WHEN json_valid(OLD.value) THEN OLD.value ELSE '{}' END,
            '$.latex_formulas'
        ), 1)
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;

PRAGMA user_version = 15;
