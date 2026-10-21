-- Appearance publication must observe justification-only edits as well.
CREATE TRIGGER configuration_justification_update
AFTER UPDATE OF value ON settings
WHEN NEW.key = 'workspace' AND json_valid(NEW.value)
    AND coalesce(json_extract(NEW.value, '$.justify'), 0)
        IS NOT coalesce(json_extract(
            CASE
                WHEN json_valid(OLD.value) THEN OLD.value
                ELSE '{}'
            END,
            '$.justify'
        ), 0)
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;

-- Recover an enabled preference saved before its publication was observed.
UPDATE configuration_state
SET revision = revision + 1
WHERE EXISTS (
    SELECT 1
    FROM settings
    WHERE key = 'workspace'
        AND CASE
            WHEN json_valid(value) THEN json_extract(value, '$.justify') = 1
            ELSE 0
        END
);

PRAGMA user_version = 13;
