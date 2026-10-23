UPDATE servers SET sync_settings = ?2 WHERE id = ?1 AND sync_settings <> ?2;
