-- Cascades and targeted cleanup must not scan unrelated study/export rows.
CREATE INDEX study_cards_comment ON study_cards(comment_id);
CREATE INDEX export_manifest_article ON export_manifest(article_id);
CREATE INDEX note_export_manifest_note ON note_export_manifest(note_id);
PRAGMA user_version = 26;
