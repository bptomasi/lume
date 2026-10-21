SELECT path, snapshot, revision <> published
FROM configuration_state
WHERE singleton = 1;
