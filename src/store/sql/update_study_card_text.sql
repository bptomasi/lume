UPDATE study_cards
SET question = ?1,
    answer = ?2
WHERE id = ?3
    AND comment_id = ?4;
