
            INSERT INTO settings (key, value)
            VALUES ('reading_session', ?1)
            ON CONFLICT (key) DO UPDATE SET value = excluded.value
