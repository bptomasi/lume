
                INSERT INTO feeds (url, title) VALUES (?1, ?2)
                ON CONFLICT (url) DO UPDATE SET enabled = 1
                WHERE feeds.enabled = 0
