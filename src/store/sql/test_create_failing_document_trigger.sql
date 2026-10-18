CREATE TRIGGER reject_document_insert
AFTER INSERT ON article_documents
BEGIN
    SELECT RAISE(ABORT, 'controlled document failure');
END;
