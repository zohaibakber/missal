CREATE TABLE template_versions (
    template_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    name TEXT NOT NULL,
    field_count INTEGER NOT NULL,
    preview TEXT NOT NULL,
    size INTEGER NOT NULL,
    object_key TEXT NOT NULL,
    published_by TEXT NOT NULL,
    published_at INTEGER NOT NULL,
    restored_from INTEGER,
    PRIMARY KEY (template_id, version)
);

INSERT INTO template_versions (
    template_id, version, name, field_count, preview, size, object_key, published_by, published_at
)
SELECT id, version, name, field_count, preview, size,
    'templates/' || id || '/' || version || '.msb', published_by, updated_at
FROM templates;
