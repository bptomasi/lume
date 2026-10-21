SELECT path, revision <> published
FROM configuration_state
WHERE singleton = 1;
