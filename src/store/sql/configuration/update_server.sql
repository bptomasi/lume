UPDATE servers
SET url = ?2,
    token_env = ?3,
    enabled = ?4,
    token = ?5
WHERE id = ?1
    AND (url <> ?2 OR token_env <> ?3 OR enabled <> ?4 OR token <> ?5);
