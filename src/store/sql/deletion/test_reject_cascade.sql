CREATE TRIGGER reject_card_cascade BEFORE DELETE ON study_cards
BEGIN
    SELECT RAISE(ABORT, 'Controlled dependent cascade failure');
END;
