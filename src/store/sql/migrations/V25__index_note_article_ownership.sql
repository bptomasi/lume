-- Permanent deletion must find historical note ownership without a library scan.
CREATE INDEX note_versions_article ON note_versions(article_id, note_id);
PRAGMA user_version = 25;
