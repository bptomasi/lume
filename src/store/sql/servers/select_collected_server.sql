SELECT EXISTS (SELECT 1 FROM servers AS s
    WHERE s.id = :server AND s.enabled = 1 AND s.url = :root
        AND s.token_env = :environment AND s.token = :token
        AND s.sync_settings = :settings);
