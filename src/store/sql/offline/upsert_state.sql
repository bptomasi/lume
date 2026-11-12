
            INSERT INTO settings (key, value)
            VALUES ('offline_package', ?1)
            ON CONFLICT (key) DO UPDATE SET value = excluded.value
