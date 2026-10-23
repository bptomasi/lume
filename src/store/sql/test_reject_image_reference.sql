CREATE TRIGGER reject_image_reference BEFORE INSERT ON image_references
BEGIN SELECT raise(ABORT, 'Injected image reference failure'); END;
