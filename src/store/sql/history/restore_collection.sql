
        INSERT INTO collections (id, name) VALUES (?1, ?2)
        ON CONFLICT (id) DO UPDATE SET name = excluded.name
