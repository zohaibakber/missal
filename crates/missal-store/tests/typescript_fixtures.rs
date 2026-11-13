mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::Scratch;
use missal_core::document::Envelope;
use missal_core::{Document, FieldReference, FirField, FirId, FirProperty, PlaceholderId};
use missal_store::Store;
use rusqlite::Connection;
use serde_json::Value;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn copy(scratch: &Scratch, source: &Path) -> PathBuf {
    let target = scratch.path(source.file_name().unwrap().to_str().unwrap());
    std::fs::copy(source, &target).unwrap();
    target
}

#[test]
fn drizzle_created_databases_open_in_every_historical_state() {
    for state in 1..=3 {
        let scratch = Scratch::new();
        let path = copy(&scratch, &fixture(&format!("drizzle-state{state}.sqlite")));
        let store = Store::open(&path).unwrap_or_else(|error| panic!("state {state}: {error:?}"));
        let fir = store.fir(FirId::new(1).unwrap()).unwrap();
        assert_eq!(fir.fir.fir_no, "7/26");
        assert_eq!(
            fir.fir.writer, "محرر صاحب",
            "state {state}: writer folded from the placeholder value"
        );
        assert_eq!(fir.fir.witness, ["گواہ"]);
        let labels: Vec<String> = store
            .placeholders()
            .unwrap()
            .into_iter()
            .map(|p| p.label)
            .collect();
        assert!(labels.contains(&"بینک".to_owned()));
        assert!(!labels.contains(&"مقدمہ نمبر".to_owned()));
        assert!(!labels.contains(&"تحریر کنندہ".to_owned()));
        assert_eq!(store.templates().unwrap().len(), 37);
        let documents = store.fir_documents(fir.id).unwrap();
        assert_eq!(documents.len(), 1);
        let document = store.fir_document(documents[0].id).unwrap();
        assert_eq!(document.revision.get(), 2);
        assert_eq!(
            document.document.field_references(),
            [
                FieldReference::Fir(FirField {
                    property: FirProperty::FirNo,
                    index: None
                }),
                FieldReference::Catalog(PlaceholderId::new(2).unwrap())
            ]
        );
    }
}

fn rows(connection: &Connection, sql: &str) -> Vec<Vec<Value>> {
    let mut statement = connection.prepare(sql).unwrap();
    let width = statement.column_count();
    statement
        .query_map([], |row| {
            (0..width)
                .map(|index| {
                    Ok(match row.get_ref(index)? {
                        rusqlite::types::ValueRef::Null => Value::Null,
                        rusqlite::types::ValueRef::Integer(int) => Value::from(int),
                        rusqlite::types::ValueRef::Real(real) => Value::from(real),
                        rusqlite::types::ValueRef::Text(text) => {
                            Value::from(String::from_utf8_lossy(text).into_owned())
                        }
                        rusqlite::types::ValueRef::Blob(_) => Value::from("<blob>"),
                    })
                })
                .collect()
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn document_value(text: &Value) -> Value {
    let envelope = Envelope::parse(text.as_str().unwrap()).unwrap();
    serde_json::json!({ "state": envelope.expanded_state(), "pageLayout": envelope.to_value()["pageLayout"] })
}

fn settings_keys(connection: &Connection) -> BTreeMap<String, String> {
    rows(connection, "SELECT id, label, source FROM placeholders")
        .into_iter()
        .filter_map(|row| {
            let source: Value = serde_json::from_str(row[2].as_str()?).ok()?;
            Some((
                source.get("setting")?.as_str()?.to_owned(),
                row[1].as_str()?.to_owned(),
            ))
        })
        .collect()
}

fn normalised_dump(connection: &Connection, include_timestamps: bool) -> Value {
    let settings = settings_keys(connection);
    let rename = |text: &str| {
        settings
            .get(text)
            .map_or_else(|| text.to_owned(), |label| format!("<setting {label}>"))
    };
    let mut dump = serde_json::Map::new();
    dump.insert(
        "fir_records".into(),
        Value::from(
            rows(connection, "SELECT * FROM fir_records ORDER BY id")
                .into_iter()
                .map(Value::from)
                .collect::<Vec<_>>(),
        ),
    );
    dump.insert(
        "placeholders".into(),
        rows(
            connection,
            "SELECT id, label, source FROM placeholders ORDER BY id",
        )
        .into_iter()
        .map(|mut row| {
            let mut source: Value = serde_json::from_str(row[2].as_str().unwrap()).unwrap();
            if let Some(setting) = source
                .get("setting")
                .and_then(Value::as_str)
                .map(str::to_owned)
            {
                source["setting"] = Value::from(rename(&setting));
            }
            row[2] = source;
            Value::from(row)
        })
        .collect(),
    );
    dump.insert("fir_placeholder_values".into(), rows(connection, "SELECT fir_id, placeholder_id, value FROM fir_placeholder_values ORDER BY fir_id, placeholder_id").into_iter().map(Value::from).collect());
    let stamp = if include_timestamps {
        ", created_at, updated_at"
    } else {
        ""
    };
    for table in ["templates", "fir_documents"] {
        let columns = if table == "templates" {
            format!(
                "id, name, revision, plain_text, preview_text, field_references, field_count, document{stamp}"
            )
        } else {
            format!(
                "id, fir_id, template_id, source_template_revision, title, revision, position, plain_text, preview_text, field_references, field_count, document{stamp}"
            )
        };
        let document_column = columns
            .split(", ")
            .position(|column| column == "document")
            .unwrap();
        dump.insert(
            table.into(),
            rows(
                connection,
                &format!("SELECT {columns} FROM {table} ORDER BY id"),
            )
            .into_iter()
            .map(|mut row| {
                row[document_column] = document_value(&row[document_column]);
                Value::from(row)
            })
            .collect(),
        );
    }
    let app_settings = rows(
        connection,
        "SELECT id, shared_placeholders, field_markers, bundled_templates_version FROM app_settings",
    );
    dump.insert(
        "app_settings".into(),
        app_settings
            .into_iter()
            .map(|mut row| {
                let shared: BTreeMap<String, String> =
                    serde_json::from_str(row[1].as_str().unwrap()).unwrap();
                row[1] = serde_json::to_value(
                    shared
                        .into_iter()
                        .map(|(key, value)| (rename(&key), value))
                        .collect::<BTreeMap<_, _>>(),
                )
                .unwrap();
                Value::from(row)
            })
            .collect(),
    );
    dump.insert(
        "__drizzle_migrations".into(),
        rows(
            connection,
            "SELECT id, hash, created_at, name FROM __drizzle_migrations ORDER BY id",
        )
        .into_iter()
        .map(Value::from)
        .collect(),
    );
    dump.insert(
        "data_migrations".into(),
        rows(connection, "SELECT migration_id, name FROM data_migrations")
            .into_iter()
            .map(Value::from)
            .collect(),
    );
    Value::Object(dump)
}

fn ts_fixtures() -> Option<PathBuf> {
    let directory = std::env::var_os("MISSAL_TS_FIXTURES").map(PathBuf::from);
    if directory.is_none() {
        eprintln!(
            "MISSAL_TS_FIXTURES not set; skipping comparison with databases upgraded by the TypeScript app"
        );
    }
    directory
}

#[test]
fn rust_upgrades_match_typescript_upgrades() {
    let Some(directory) = ts_fixtures() else {
        return;
    };
    for state in 1..=3 {
        let scratch = Scratch::new();
        let ours = copy(&scratch, &directory.join(format!("state{state}.sqlite")));
        drop(Store::open(&ours).unwrap());
        let theirs = copy(
            &scratch,
            &directory.join(format!("state{state}-upgraded-by-ts.sqlite")),
        );
        let ours = normalised_dump(&Connection::open(&ours).unwrap(), false);
        let theirs = normalised_dump(&Connection::open(&theirs).unwrap(), false);
        for (table, expected) in theirs.as_object().unwrap() {
            assert_eq!(
                &ours[table], expected,
                "state {state}: table {table} differs"
            );
        }
        eprintln!("state {state}: every table matches the TypeScript upgrade");
    }
}

#[test]
fn typescript_seeded_database_reads_back_with_identical_projections() {
    let Some(directory) = ts_fixtures() else {
        return;
    };
    let scratch = Scratch::new();
    let path = copy(&scratch, &directory.join("seeded.sqlite"));
    let before = normalised_dump(&Connection::open(&path).unwrap(), true);
    let store = Store::open(&path).unwrap();
    assert_eq!(store.templates().unwrap().len(), 36);
    let firs = store.firs().unwrap();
    assert_eq!(firs.len(), 1);
    let workspace = store.fir_workspace(firs[0].id).unwrap();
    assert_eq!(workspace.values.fir.fir.fir_no, "12/26");
    assert_eq!(workspace.documents.len(), 3);
    assert_eq!(workspace.values.overrides[0].value, "HBL");
    assert_eq!(store.settings().unwrap().field_markers.open(), "{{");
    let globals = store.global_placeholders().unwrap();
    assert_eq!(globals.len(), 6);
    assert_eq!(globals[0].value, "قدر 0");
    let connection = store.into_connection();
    assert_eq!(
        normalised_dump(&connection, true),
        before,
        "opening a current database changes nothing"
    );
    let mut compared = 0;
    for table in ["templates", "fir_documents"] {
        for row in rows(
            &connection,
            &format!(
                "SELECT document, plain_text, preview_text, field_references, field_count FROM {table}"
            ),
        ) {
            let document = Document::decode(row[0].as_str().unwrap()).unwrap();
            let projections = document.projections();
            assert_eq!(projections.plain_text, row[1].as_str().unwrap());
            assert_eq!(projections.preview_text, row[2].as_str().unwrap());
            assert_eq!(
                projections.field_references_json(),
                row[3].as_str().unwrap()
            );
            assert_eq!(projections.field_count as i64, row[4].as_i64().unwrap());
            let reencoded = Document::decode(&document.encode()).unwrap();
            assert_eq!(reencoded, document);
            compared += 1;
        }
    }
    assert_eq!(compared, 39);

    let fresh = scratch.path("fresh.sqlite");
    drop(Store::open(&fresh).unwrap());
    let fresh = Connection::open(&fresh).unwrap();
    let projection_rows = |connection: &Connection| {
        rows(connection, "SELECT name, revision, plain_text, preview_text, field_references, field_count, document FROM templates ORDER BY id")
            .into_iter()
            .map(|mut row| {
                row[6] = document_value(&row[6]);
                row
            })
            .collect::<Vec<_>>()
    };
    let ts_seed = Connection::open(directory.join("state3-upgraded-by-ts.sqlite")).unwrap();
    let theirs: Vec<_> = projection_rows(&ts_seed)
        .into_iter()
        .filter(|row| row[0] != "پرانا سانچہ")
        .collect();
    let ours = projection_rows(&fresh);
    assert_eq!(ours.len(), 36);
    assert_eq!(theirs.len(), 36);
    let theirs_seed = projection_rows(&Connection::open(directory.join("seeded.sqlite")).unwrap());
    for (ours, theirs) in ours.iter().zip(&theirs_seed) {
        assert_eq!(
            ours, theirs,
            "seeded template {} differs from the TypeScript seed",
            ours[0]
        );
    }
    eprintln!(
        "compared {compared} stored documents and 36 seeded templates with the TypeScript app"
    );
}
