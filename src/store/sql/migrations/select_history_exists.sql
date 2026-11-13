SELECT EXISTS (
    SELECT 1 FROM sqlite_schema
    WHERE type = 'table' AND name = 'refinery_schema_history'
);
