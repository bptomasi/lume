ALTER TABLE articles
ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0
    CHECK (deleted IN (0, 1));

CREATE INDEX articles_visibility_order
ON articles (deleted, published DESC, id DESC);

PRAGMA user_version = 3;
