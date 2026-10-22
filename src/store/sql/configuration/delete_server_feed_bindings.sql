-- Server-owned feeds are projected from the configured server, not TOML feeds.
DELETE FROM configuration_bindings
WHERE kind = 'feed'
    AND entity IN (SELECT feed_id FROM server_feeds);
