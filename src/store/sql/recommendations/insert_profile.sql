INSERT INTO settings (key, value)
VALUES ('recommendation_profile', ?1)
ON CONFLICT (key) DO UPDATE SET value = excluded.value;
