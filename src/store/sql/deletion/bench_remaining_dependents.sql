SELECT (SELECT count(*) FROM comments) + (SELECT count(*) FROM study_cards)
    + (SELECT count(*) FROM export_manifest)
    + (SELECT count(*) FROM note_export_manifest);
