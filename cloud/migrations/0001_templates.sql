CREATE TABLE templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version INTEGER NOT NULL,
    field_count INTEGER NOT NULL,
    preview TEXT NOT NULL,
    size INTEGER NOT NULL,
    published_by TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
