SELECT substr(key, length('collector_retry:') + 1), value
FROM settings
WHERE key GLOB 'collector_retry:*';
