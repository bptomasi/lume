
        DELETE FROM undo_log
        WHERE id NOT IN (SELECT id FROM undo_log ORDER BY id DESC LIMIT 256)
