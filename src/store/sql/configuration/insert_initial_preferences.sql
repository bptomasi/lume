INSERT INTO settings (key, value)
VALUES ('workspace', ?1), ('layouts', ?2)
ON CONFLICT (key) DO NOTHING;
