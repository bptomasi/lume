UPDATE configuration_state
SET path = '',
    snapshot = '',
    published = 0,
    revision = revision + 1
WHERE singleton = 1;
