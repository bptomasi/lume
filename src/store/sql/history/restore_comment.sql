
        INSERT INTO comments (
            id, article_id, body, created, quote, anchor, source, source_revision
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT (id) DO UPDATE SET
            body = excluded.body,
            quote = excluded.quote,
            anchor = excluded.anchor,
            source = excluded.source,
            source_revision = excluded.source_revision
        WHERE comments.article_id = excluded.article_id
