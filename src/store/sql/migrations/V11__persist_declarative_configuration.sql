-- Configuration definitions are soft-archived; personal memberships survive.
ALTER TABLE collections ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE smart_lists ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE note_collections ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1;

CREATE TABLE configuration_state (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    path TEXT NOT NULL DEFAULT '',
    revision INTEGER NOT NULL DEFAULT 0,
    published INTEGER NOT NULL DEFAULT 0,
    snapshot TEXT NOT NULL DEFAULT ''
);

INSERT INTO configuration_state (singleton) VALUES (1);

CREATE TABLE configuration_bindings (
    kind TEXT NOT NULL,
    entity INTEGER NOT NULL,
    key TEXT NOT NULL,
    present INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (kind, entity),
    UNIQUE (kind, key)
);

-- Preserve the former implicit profile only for an existing populated library.
-- New libraries have no subscriptions at migration time and remain neutral.
INSERT INTO settings (key, value)
SELECT 'recommendation_profile', '{
  "interests": [
    {
      "name": "Kubernetes internals",
      "weight": 8,
      "query": "kubernetes OR kubelet OR etcd OR \"kube proxy\" OR \"container runtime\""
    },
    {
      "name": "Networking and dataplanes",
      "weight": 9,
      "query": "networking OR cilium OR CNI OR BGP OR conntrack OR \"connection tracking\" OR \"network policy\" OR dataplane OR XDP"
    },
    {
      "name": "Kernel and systems",
      "weight": 8,
      "query": "kernel OR syscall OR eBPF OR \"memory management\" OR scheduling OR \"systems programming\""
    },
    {
      "name": "Distributed systems",
      "weight": 9,
      "query": "\"distributed systems\" OR consensus OR raft OR replication OR consistency OR \"fault tolerance\""
    },
    {
      "name": "Architecture",
      "weight": 8,
      "query": "architecture OR microservices OR \"system design\" OR \"event driven\""
    },
    {
      "name": "Data and databases",
      "weight": 8,
      "query": "database OR databases OR postgres OR PostgreSQL OR Kafka OR CDC OR \"data pipeline\" OR transaction OR indexing"
    },
    {
      "name": "Security and identity",
      "weight": 8,
      "query": "security OR vulnerability OR exploit OR authentication OR authorization OR OAuth OR OpenID OR OIDC OR cryptography"
    },
    {
      "name": "Platform and reliability",
      "weight": 7,
      "query": "SRE OR DevOps OR \"platform engineering\" OR observability OR postmortem OR Terraform OR GitOps OR \"incident analysis\""
    },
    {
      "name": "AI and models",
      "weight": 6,
      "query": "transformer OR transformers OR LLM OR LLMs OR inference OR \"language model\" OR \"machine learning\" OR \"model training\""
    }
  ],
  "exclude": "",
  "minutes": null,
  "include_read": false,
  "sources": {}
}'
WHERE EXISTS (SELECT 1 FROM feeds)
    AND NOT EXISTS (
        SELECT 1 FROM settings WHERE key = 'recommendation_profile'
    );

CREATE TRIGGER configuration_feeds_insert
AFTER INSERT ON feeds
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;

CREATE TRIGGER configuration_feeds_delete
AFTER DELETE ON feeds
BEGIN
    UPDATE configuration_state SET revision = revision + 1;

    DELETE FROM configuration_bindings
    WHERE kind = 'feed' AND entity = OLD.id;
END;

CREATE TRIGGER configuration_feeds_update
AFTER UPDATE ON feeds
WHEN NEW.url IS NOT OLD.url
    OR NEW.enabled IS NOT OLD.enabled
    OR NEW.article_selector IS NOT OLD.article_selector
BEGIN
    UPDATE configuration_state SET revision = revision + 1;

    UPDATE configuration_bindings SET present = 1
    WHERE kind = 'feed' AND entity = NEW.id AND NEW.enabled = 1;
END;

CREATE TRIGGER configuration_collections_insert
AFTER INSERT ON collections
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;

CREATE TRIGGER configuration_collections_delete
AFTER DELETE ON collections
BEGIN
    UPDATE configuration_state SET revision = revision + 1;

    DELETE FROM configuration_bindings
    WHERE kind = 'folder' AND entity = OLD.id;
END;

CREATE TRIGGER configuration_collections_update
AFTER UPDATE ON collections
WHEN NEW.name IS NOT OLD.name
    OR NEW.enabled IS NOT OLD.enabled
BEGIN
    UPDATE configuration_state SET revision = revision + 1;

    UPDATE configuration_bindings SET present = 1
    WHERE kind = 'folder' AND entity = NEW.id AND NEW.enabled = 1;
END;

CREATE TRIGGER configuration_smart_lists_insert
AFTER INSERT ON smart_lists
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;

CREATE TRIGGER configuration_smart_lists_delete
AFTER DELETE ON smart_lists
BEGIN
    UPDATE configuration_state SET revision = revision + 1;

    DELETE FROM configuration_bindings
    WHERE kind = 'smart' AND entity = OLD.id;
END;

CREATE TRIGGER configuration_smart_lists_update
AFTER UPDATE ON smart_lists
WHEN NEW.name IS NOT OLD.name
    OR NEW.definition IS NOT OLD.definition
    OR NEW.enabled IS NOT OLD.enabled
BEGIN
    UPDATE configuration_state SET revision = revision + 1;

    UPDATE configuration_bindings SET present = 1
    WHERE kind = 'smart' AND entity = NEW.id AND NEW.enabled = 1;
END;

CREATE TRIGGER configuration_note_collections_insert
AFTER INSERT ON note_collections
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;

CREATE TRIGGER configuration_note_collections_delete
AFTER DELETE ON note_collections
BEGIN
    UPDATE configuration_state SET revision = revision + 1;

    DELETE FROM configuration_bindings
    WHERE kind = 'notes' AND entity = OLD.id;
END;

CREATE TRIGGER configuration_note_collections_update
AFTER UPDATE ON note_collections
WHEN NEW.name IS NOT OLD.name
    OR NEW.query IS NOT OLD.query
    OR NEW.enabled IS NOT OLD.enabled
BEGIN
    UPDATE configuration_state SET revision = revision + 1;

    UPDATE configuration_bindings SET present = 1
    WHERE kind = 'notes' AND entity = NEW.id AND NEW.enabled = 1;
END;

CREATE TRIGGER configuration_settings_insert
AFTER INSERT ON settings
WHEN (NEW.key = 'recommendation_profile')
    OR (NEW.key = 'workspace' AND json_valid(NEW.value))
    OR (NEW.key = 'layouts' AND json_valid(NEW.value))
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;

CREATE TRIGGER configuration_settings_update
AFTER UPDATE ON settings
WHEN (NEW.key = 'recommendation_profile' AND NEW.value IS NOT OLD.value)
    OR (NEW.key = 'workspace' AND json_valid(NEW.value) AND (
        json_extract(NEW.value,
            '$.theme',
            '$.mouse',
            '$.keybindings',
            '$.layout',
            '$.panel_order',
            '$.show_library',
            '$.show_articles',
            '$.show_reader',
            '$.show_annotations',
            '$.library_width',
            '$.article_width',
            '$.annotation_width',
            '$.reading_width',
            '$.fullscreen_width',
            '$.compact',
            '$.zen',
            '$.fullscreen'
        ) IS NOT json_extract(
            CASE
                WHEN json_valid(OLD.value) THEN OLD.value
                ELSE '{}'
            END,
            '$.theme',
            '$.mouse',
            '$.keybindings',
            '$.layout',
            '$.panel_order',
            '$.show_library',
            '$.show_articles',
            '$.show_reader',
            '$.show_annotations',
            '$.library_width',
            '$.article_width',
            '$.annotation_width',
            '$.reading_width',
            '$.fullscreen_width',
            '$.compact',
            '$.zen',
            '$.fullscreen'
        )
    ))
    OR (NEW.key = 'layouts' AND json_valid(NEW.value) AND (
        json_extract(NEW.value, '$.profiles')
        IS NOT json_extract(
            CASE
                WHEN json_valid(OLD.value) THEN OLD.value
                ELSE '{}'
            END, '$.profiles')
    ))
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;

PRAGMA user_version = 11;
