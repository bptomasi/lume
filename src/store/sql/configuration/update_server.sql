UPDATE servers
SET url = ?2,
    token_env = ?3,
    enabled = ?4
WHERE id = ?1
    AND (url <> ?2 OR token_env <> ?3 OR enabled <> ?4);
