UPDATE configuration_state
SET path = ?1,
    snapshot = ?2,
    published = revision
WHERE singleton = 1;
