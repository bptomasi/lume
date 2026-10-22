-- Optional remote reader servers; local feeds and personal state stay authoritative.
CREATE TABLE servers (
    id INTEGER PRIMARY KEY,
    url TEXT NOT NULL UNIQUE,
    token_env TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    error TEXT NOT NULL DEFAULT '',
    attempted_at INTEGER NOT NULL DEFAULT 0,
    succeeded_at INTEGER NOT NULL DEFAULT 0
);

ALTER TABLE feeds ADD COLUMN server_id INTEGER REFERENCES servers(id);

CREATE INDEX feeds_server ON feeds(server_id);

CREATE TABLE server_feeds (
    server_id INTEGER NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    remote_id INTEGER NOT NULL,
    remote_url TEXT NOT NULL DEFAULT '',
    feed_id INTEGER NOT NULL UNIQUE REFERENCES feeds(id) ON DELETE CASCADE,
    initialized INTEGER NOT NULL DEFAULT 0,
    last_changed INTEGER NOT NULL DEFAULT 0,
    present INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (server_id, remote_id)
);

CREATE TABLE server_entries (
    server_id INTEGER NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    remote_id INTEGER NOT NULL,
    article_id INTEGER NOT NULL UNIQUE REFERENCES articles(id) ON DELETE CASCADE,
    PRIMARY KEY (server_id, remote_id)
);

PRAGMA user_version = 17;
