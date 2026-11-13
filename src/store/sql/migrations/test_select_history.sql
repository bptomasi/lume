SELECT version, name, applied_on, checksum
FROM refinery_schema_history
WHERE version <= ?
ORDER BY version;
