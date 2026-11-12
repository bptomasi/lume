
            INSERT INTO export_manifest (namespace, article_id, content)
            VALUES (?1, ?2, ?3)
            ON CONFLICT (namespace, article_id) DO UPDATE SET
                content = excluded.content
