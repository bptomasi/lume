
            SELECT count(*)
            FROM note_collections
            WHERE enabled = 1 AND name <> ?1 COLLATE NOCASE
