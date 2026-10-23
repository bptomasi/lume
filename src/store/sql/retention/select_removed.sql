SELECT EXISTS(SELECT 1 FROM retention_tombstones
    WHERE feed_id = ?1 AND external_id = ?2)
OR (?3 <> '' AND EXISTS(SELECT 1 FROM retention_tombstones
    WHERE feed_id = ?1 AND canonical_key <> '' AND canonical_key = ?3));
