SELECT EXISTS(SELECT 1 FROM retention_tombstones WHERE feed_id = ?1);
