CREATE INDEX articles_read_visibility
ON articles (deleted, unread);

PRAGMA user_version = 8;
