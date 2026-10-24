-- Missing targets can be retried after their acknowledged permanent deletion.
SELECT coalesce((SELECT deleted FROM articles WHERE id = ?1), 1);
