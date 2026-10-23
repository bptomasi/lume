UPDATE server_feeds SET present = 0
WHERE server_id = ?1 AND present <> 0
    AND remote_id NOT IN (SELECT value FROM json_each(?2));
