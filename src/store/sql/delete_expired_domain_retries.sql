DELETE FROM settings
WHERE key GLOB 'collector_retry:*'
    AND CAST(value AS INTEGER) <= ?1;
