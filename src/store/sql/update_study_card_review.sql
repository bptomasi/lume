UPDATE study_cards
SET
    due_at = ?1,
    interval_days = ?2,
    reviews = reviews + 1
WHERE id = ?3;
