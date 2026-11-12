
            INSERT INTO feeds (url, title) VALUES (?1, ?1)
            ON CONFLICT (url) DO UPDATE SET enabled = 1
