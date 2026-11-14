mod common;

use common::{Scratch, count, drizzle_database};
use missal_core::{Document, FieldReference, FirProperty, RepositoryError};
use missal_store::{Store, TemplateBundle};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

fn reopen(path: &std::path::Path) -> Connection {
    Connection::open(path).unwrap()
}

fn assert_fully_migrated(connection: &Connection) {
    let names: Vec<String> = connection
        .prepare("SELECT name FROM __drizzle_migrations ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        names,
        common::MIGRATIONS
            .iter()
            .map(|(name, ..)| name.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        count(
            connection,
            "SELECT count(*) FROM data_migrations WHERE migration_id = 1 AND name = 'fold_placeholders_into_fir_fields'"
        ),
        1
    );
    assert_eq!(
        count(
            connection,
            "SELECT count(*) FROM pragma_table_info('fir_records') WHERE name IN ('nic', 'mobile')"
        ),
        0
    );
    assert_eq!(
        count(
            connection,
            "SELECT count(*) FROM pragma_table_info('fir_records') WHERE name = 'sim_2'"
        ),
        1
    );
}

#[test]
fn a_new_file_is_created_migrated_and_seeded() {
    let scratch = Scratch::new();
    let path = scratch.path("missal.sqlite");
    let store = Store::open(&path).unwrap();
    assert_eq!(store.templates().unwrap().len(), 36);
    assert_eq!(store.placeholders().unwrap().len(), 40);
    assert_eq!(store.global_placeholders().unwrap().len(), 5);
    let connection = store.into_connection();
    assert_eq!(count(&connection, "PRAGMA foreign_keys"), 1);
    assert_eq!(count(&connection, "PRAGMA synchronous"), 1);
    let journal: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(journal, "wal");
    assert_eq!(
        count(
            &connection,
            "SELECT bundled_templates_version FROM app_settings"
        ),
        1
    );
    assert_fully_migrated(&connection);
}

#[test]
fn opening_again_changes_nothing() {
    let scratch = Scratch::new();
    let path = scratch.path("missal.sqlite");
    drop(Store::open(&path).unwrap());
    let snapshot = |connection: &Connection| {
        (
            count(connection, "SELECT count(*) FROM templates"),
            count(connection, "SELECT count(*) FROM placeholders"),
            count(connection, "SELECT count(*) FROM __drizzle_migrations"),
            count(connection, "SELECT count(*) FROM data_migrations"),
            count(connection, "SELECT sum(length(document)) FROM templates"),
        )
    };
    let before = snapshot(&reopen(&path));
    for _ in 0..2 {
        drop(Store::open(&path).unwrap());
    }
    assert_eq!(snapshot(&reopen(&path)), before);
}

fn legacy_fir(connection: &Connection, applied: usize) {
    let (columns, values) = if applied < 3 {
        (", nic, mobile", ", '123', '0300'")
    } else {
        ("", "")
    };
    connection
        .execute(
            &format!("INSERT INTO fir_records (fir_no, date, offence, accused, witness, zimni, incident_date, arrest_date, investigation_officer, status{columns}) VALUES ('7/26', '01-01-2026', 'چوری', '[\"علی\"]', '[]', '[]', '01-01-2026', '', 'انسپکٹر', 'Open'{values})"),
            [],
        )
        .unwrap();
}

fn legacy_template(connection: &Connection, name: &str, placeholder: i64) {
    let document = json!({
        "format": "missal-lexical",
        "version": 1,
        "state": {"root": {"type": "root", "children": [{"type": "paragraph", "children": [
            {"type": "text", "text": "مقدمہ ", "style": "color: #333"},
            {"type": "field", "reference": {"_tag": "CatalogField", "id": placeholder}}
        ]}]}}
    });
    connection
        .execute(
            "INSERT INTO templates (name, document, revision, plain_text, preview_text, field_references, field_count, created_at, updated_at) VALUES (?1, ?2, 3, 'مقدمہ', 'مقدمہ', ?3, 1, '2026-10-01T00:00:00.000Z', '2026-10-01T00:00:00.000Z')",
            params![name, document.to_string(), json!([{"_tag": "CatalogField", "id": placeholder}]).to_string()],
        )
        .unwrap();
}

#[test]
fn each_historical_schema_state_is_upgraded_in_place() {
    for (applied, legacy_table) in [(1, false), (2, false), (3, false), (1, true), (3, true)] {
        let scratch = Scratch::new();
        let path = scratch.path("old.sqlite");
        {
            let connection = drizzle_database(&path, applied, legacy_table);
            legacy_fir(&connection, applied);
            connection
                .execute("INSERT INTO placeholders (label, source) VALUES ('مقدمہ نمبر', '{\"_tag\":\"Custom\"}'), ('بینک', '{\"_tag\":\"Custom\"}')", [])
                .unwrap();
            connection
                .execute("INSERT INTO fir_placeholder_values (fir_id, placeholder_id, value, updated_at) VALUES (1, 2, 'HBL', '2026-10-01T00:00:00.000Z')", [])
                .unwrap();
            legacy_template(&connection, "پرانا", 1);
        }
        let store = Store::open(&path)
            .unwrap_or_else(|error| panic!("state {applied}/{legacy_table}: {error:?}"));
        let fir = store.fir(missal_core::FirId::new(1).unwrap()).unwrap();
        assert_eq!(fir.fir.fir_no, "7/26");
        assert_eq!(fir.fir.investigation_officer, "انسپکٹر");
        assert_eq!(fir.fir.complainant, "");
        let templates = store.templates().unwrap();
        assert_eq!(templates.len(), 37, "state {applied}");
        let old = templates
            .iter()
            .find(|template| template.name == "پرانا")
            .unwrap();
        assert_eq!(old.revision.get(), 3);
        let record = store.template(old.id).unwrap();
        assert_eq!(
            record.document.field_references(),
            [FieldReference::Fir(missal_core::FirField {
                property: FirProperty::FirNo,
                index: None
            })]
        );
        let labels: Vec<String> = store
            .placeholders()
            .unwrap()
            .into_iter()
            .map(|placeholder| placeholder.label)
            .collect();
        assert!(!labels.iter().any(|label| label == "مقدمہ نمبر"));
        assert_eq!(labels.iter().filter(|label| *label == "بینک").count(), 1);
        assert_eq!(store.placeholder_values(fir.id).unwrap().len(), 1);
        let connection = store.into_connection();
        let stored: String = connection
            .query_row(
                "SELECT document FROM templates WHERE name = 'پرانا'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let stored: Value = serde_json::from_str(&stored).unwrap();
        assert_eq!(stored["version"], 2);
        assert_eq!(stored["styles"], json!(["color: #333"]));
        let field_references: String = connection
            .query_row(
                "SELECT field_references FROM templates WHERE name = 'پرانا'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            field_references,
            r#"[{"_tag":"FirField","property":"fir_no"}]"#
        );
        if legacy_table {
            assert_eq!(
                count(&connection, "SELECT count(*) FROM __drizzle_migrations"),
                3
            );
        } else {
            assert_fully_migrated(&connection);
        }
    }
}

#[test]
fn placeholder_values_fold_into_empty_fir_columns() {
    let scratch = Scratch::new();
    let path = scratch.path("fold.sqlite");
    {
        let connection = drizzle_database(&path, 3, false);
        for number in ["1/26", "2/26"] {
            connection
                .execute(
                    "INSERT INTO fir_records (fir_no, date, offence, accused, incident_date, status, writer) VALUES (?1, '01-01-2026', 'x', '[\"a\"]', '01-01-2026', 'Open', ?2)",
                    params![number, if number == "1/26" { "" } else { "موجود" }],
                )
                .unwrap();
        }
        connection
            .execute("INSERT INTO placeholders (label, source) VALUES ('تحریر_کنندہ', '{\"_tag\":\"Custom\"}'), ('گواہان 1', '{\"_tag\":\"Custom\"}')", [])
            .unwrap();
        connection
            .execute("INSERT INTO fir_placeholder_values (fir_id, placeholder_id, value, updated_at) VALUES (1, 1, 'نیا', 'x'), (2, 1, 'نیا', 'x'), (1, 2, 'گواہ', 'x')", [])
            .unwrap();
    }
    let store = Store::open(&path).unwrap();
    let first = store.fir(missal_core::FirId::new(1).unwrap()).unwrap();
    let second = store.fir(missal_core::FirId::new(2).unwrap()).unwrap();
    assert_eq!(first.fir.writer, "نیا");
    assert_eq!(second.fir.writer, "موجود");
    assert!(first.fir.witness.is_empty());
    let connection = store.into_connection();
    assert_eq!(
        count(&connection, "SELECT count(*) FROM fir_placeholder_values"),
        0
    );
    assert_eq!(
        count(
            &connection,
            "SELECT count(*) FROM placeholders WHERE label IN ('تحریر_کنندہ', 'گواہان 1')"
        ),
        0
    );
}

fn assert_refused(path: &std::path::Path, setup: impl FnOnce(&Connection)) {
    {
        let connection = Connection::open(path).unwrap();
        setup(&connection);
    }
    let before = std::fs::read(path).unwrap();
    let error = Store::open(path).err().expect("opening must be refused");
    assert!(
        matches!(error, RepositoryError::Storage { .. }),
        "{error:?}"
    );
    assert_eq!(
        std::fs::read(path).unwrap(),
        before,
        "a refused database must not be modified"
    );
}

#[test]
fn unrecognised_databases_are_refused_without_changes() {
    let scratch = Scratch::new();
    assert_refused(&scratch.path("foreign.sqlite"), |connection| {
        connection
            .execute_batch("CREATE TABLE notes (body text); INSERT INTO notes VALUES ('keep me');")
            .unwrap();
    });
    let unknown = scratch.path("unknown-migration.sqlite");
    drop(drizzle_database(&unknown, 3, false));
    assert_refused(&unknown, |connection| {
        connection
            .execute("INSERT INTO __drizzle_migrations (hash, created_at, name) VALUES ('x', 1, '20260910191757_sharp_masked_marvel')", [])
            .unwrap();
    });
    let mismatch = scratch.path("mismatch.sqlite");
    drop(drizzle_database(&mismatch, 1, false));
    assert_refused(&mismatch, |connection| {
        connection
            .execute_batch("ALTER TABLE fir_records ADD complainant text DEFAULT '' NOT NULL")
            .unwrap();
    });
    let gap = scratch.path("gap.sqlite");
    drop(drizzle_database(&gap, 3, false));
    assert_refused(&gap, |connection| {
        connection
            .execute(
                "DELETE FROM __drizzle_migrations WHERE name = '20261028010119_chemical_nocturne'",
                [],
            )
            .unwrap();
    });
    let newer = scratch.path("newer.sqlite");
    drop(Store::open(&newer).unwrap());
    assert_refused(&newer, |connection| {
        connection.pragma_update(None, "user_version", 99).unwrap();
    });
}

#[test]
fn seeding_runs_once_per_bundle_version() {
    let scratch = Scratch::new();
    let path = scratch.path("seed.sqlite");
    let store = Store::open(&path).unwrap();
    let mut store = store;
    let first = store.templates().unwrap().into_iter().next().unwrap();
    store.remove_template(first.id).unwrap();
    let bank = store
        .placeholders()
        .unwrap()
        .into_iter()
        .find(|placeholder| placeholder.label == "بینک")
        .unwrap();
    store.rename_placeholder(bank.id, "بی نک").unwrap();
    drop(store);
    let store = Store::open(&path).unwrap();
    assert_eq!(store.templates().unwrap().len(), 35);
    drop(store);

    let fields =
        include_str!("../../../templates/fields.json").replace("\"version\": 1", "\"version\": 2");
    let bundle = TemplateBundle::new(
        &fields,
        include_bytes!("../../../templates/bundle.json.gz").to_vec(),
    )
    .unwrap();
    let store = Store::open_with(&path, &bundle).unwrap();
    assert_eq!(store.templates().unwrap().len(), 36);
    let placeholders = store.placeholders().unwrap();
    assert_eq!(
        placeholders.len(),
        40,
        "labels matching by lookup key are reused"
    );
    assert!(
        placeholders
            .iter()
            .any(|placeholder| placeholder.label == "بی نک")
    );
    let restored = store
        .templates()
        .unwrap()
        .into_iter()
        .find(|template| template.name == first.name)
        .unwrap();
    let document = store.template(restored.id).unwrap().document;
    assert!(
        document.field_references().iter().all(
            |reference| !matches!(reference, FieldReference::Unresolved(text) if text == "بینک")
        )
    );
    assert_eq!(store.settings().unwrap().shared_placeholders.len(), 0);
    let _ = Document::empty();
}
