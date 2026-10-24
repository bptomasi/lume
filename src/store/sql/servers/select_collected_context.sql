SELECT EXISTS (
    SELECT 1 FROM server_feeds AS sf
    JOIN feeds AS f ON f.id = sf.feed_id
    JOIN servers AS s ON s.id = sf.server_id
    WHERE f.id = :feed AND f.enabled = 1 AND s.enabled = 1 AND sf.present = 1
        AND sf.server_id = :server AND sf.remote_id = :remote
        AND s.url = :root AND s.token_env = :environment AND s.token = :token
        AND s.sync_settings = :settings
        AND sf.initialized = json_extract(:cursor, '$.initialized')
        AND sf.last_changed = json_extract(:cursor, '$.changed')
        AND sf.sync_after_id = json_extract(:cursor, '$.after')
        AND sf.sync_latest = json_extract(:cursor, '$.latest')
        AND sf.history_before_id = json_extract(:cursor, '$.before')
        AND sf.history_loaded = json_extract(:cursor, '$.loaded')
        AND sf.history_total = json_extract(:cursor, '$.total')
);
