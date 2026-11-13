use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use flate2::read::GzDecoder;
use missal_core::document::{Envelope, project_state};
use missal_core::{Document, PlaceholderId, field_lookup_key, format_date};
use serde_json::Value;

const BUNDLE: &[u8] = include_bytes!("../../../templates/bundle.json.gz");
const FIELDS: &str = include_str!("../../../templates/fields.json");

fn bundle() -> Vec<(String, Envelope)> {
    let mut json = String::new();
    GzDecoder::new(BUNDLE).read_to_string(&mut json).unwrap();
    let Value::Array(items) = serde_json::from_str(&json).unwrap() else {
        panic!("bundle is not an array");
    };
    items
        .into_iter()
        .map(|item| {
            let name = item["name"].as_str().unwrap().to_owned();
            (
                name,
                Envelope::from_value(item["document"].clone()).unwrap(),
            )
        })
        .collect()
}

fn labels() -> Vec<String> {
    let fields: Value = serde_json::from_str(FIELDS).unwrap();
    ["global", "custom"]
        .iter()
        .flat_map(|key| fields[key].as_array().unwrap().clone())
        .map(|label| label.as_str().unwrap().to_owned())
        .collect()
}

fn ids() -> HashMap<String, PlaceholderId> {
    labels()
        .iter()
        .enumerate()
        .map(|(index, label)| {
            (
                field_lookup_key(label),
                PlaceholderId::new(index as i64 + 1).unwrap(),
            )
        })
        .collect()
}

#[test]
fn every_bundled_template_round_trips_losslessly() {
    let bundle = bundle();
    assert_eq!(bundle.len(), 36);
    for (name, envelope) in &bundle {
        let document = Document::from_envelope(envelope).unwrap();
        let encoded = document.to_envelope();
        assert_eq!(
            encoded.expanded_state(),
            envelope.expanded_state(),
            "{name}: state changed"
        );
        assert_eq!(
            encoded.page_layout, envelope.page_layout,
            "{name}: layout changed"
        );
        let decoded = Document::decode(&document.encode()).unwrap();
        assert_eq!(decoded, document, "{name}: model changed");
        assert_eq!(
            document.projections(),
            project_state(&envelope.state),
            "{name}"
        );
    }
}

#[test]
fn bundled_templates_are_fully_typed() {
    use missal_core::document::{Block, Child, Inline};
    fn inline_typed(inline: &Inline) -> bool {
        !matches!(inline, Inline::Other(_))
    }
    fn block_typed(block: &Block) -> bool {
        match block {
            Block::Other(_) => false,
            Block::Paragraph(paragraph) => paragraph.children.iter().all(inline_typed),
            Block::Table(table) => table.rows.iter().all(|row| match row {
                Child::Node(row) => row.cells.iter().all(|cell| match cell {
                    Child::Node(cell) => cell.blocks.iter().all(block_typed),
                    Child::Other(_) => false,
                }),
                Child::Other(_) => false,
            }),
            _ => true,
        }
    }
    let mut fields = 0;
    for (name, envelope) in bundle() {
        let mut document = Document::from_envelope(&envelope).unwrap();
        assert!(document.blocks.iter().all(block_typed), "{name}");
        document.for_each_field_mut(&mut |field| {
            assert!(field.extra.get("reference").is_none());
            fields += 1;
        });
    }
    assert_eq!(fields, 8887);
}

#[test]
fn typed_binding_matches_json_binding_and_unbinding_restores_tokens() {
    let labels = labels();
    let ids = ids();
    let label_of = |id: PlaceholderId| labels.get(id.get() as usize - 1).cloned();
    for (name, envelope) in bundle() {
        let original = Document::from_envelope(&envelope).unwrap();
        let mut document = original.clone();
        document.bind_catalog_fields(&ids);
        let mut raw_bound = envelope.expanded_state();
        missal_core::field::map_references(&mut raw_bound, &mut |reference| {
            missal_core::field::bind_reference_json(reference, &ids)
        });
        assert_eq!(document.to_envelope().expanded_state(), raw_bound, "{name}");
        assert!(
            document
                .field_references()
                .iter()
                .all(|reference| !matches!(reference, missal_core::FieldReference::Unresolved(text) if ids.contains_key(&field_lookup_key(text)))),
            "{name}"
        );
        document.unbind_catalog_fields(&label_of);
        assert_eq!(
            document.projections().plain_text,
            original.projections().plain_text
        );
        assert!(
            document
                .field_references()
                .iter()
                .all(|reference| !matches!(reference, missal_core::FieldReference::Catalog(_))),
            "{name}"
        );
    }
}

#[test]
fn matches_typescript_ground_truth() {
    let Ok(path) = std::env::var("MISSAL_TS_TRUTH") else {
        eprintln!("MISSAL_TS_TRUTH not set; skipping comparison with the TypeScript app");
        return;
    };
    let truth: Value = serde_json::from_slice(&std::fs::read(Path::new(&path)).unwrap()).unwrap();
    let labels = labels();
    let ids = ids();
    let label_of = |id: PlaceholderId| labels.get(id.get() as usize - 1).cloned();
    let check = |document: &Document, expected: &Value, what: &str| {
        let projections = document.projections();
        assert_eq!(
            projections.plain_text,
            expected["plainText"].as_str().unwrap(),
            "{what}"
        );
        assert_eq!(
            projections.preview_text,
            expected["previewText"].as_str().unwrap(),
            "{what}"
        );
        assert_eq!(
            projections.field_references_json(),
            expected["fieldReferences"].as_str().unwrap(),
            "{what}"
        );
        assert_eq!(
            projections.field_count as u64,
            expected["fieldCount"].as_u64().unwrap(),
            "{what}"
        );
    };
    let templates = truth["templates"].as_array().unwrap();
    let bundle = bundle();
    assert_eq!(templates.len(), bundle.len());
    let mut compared = 0;
    for ((name, envelope), expected) in bundle.iter().zip(templates) {
        assert_eq!(name, expected["name"].as_str().unwrap());
        let mut document = Document::from_envelope(envelope).unwrap();
        check(&document, &expected["raw"], name);
        document.bind_catalog_fields(&ids);
        check(&document, &expected["bound"], name);
        assert_eq!(
            document.to_envelope().expanded_state(),
            expected["boundState"],
            "{name}: bound state"
        );
        document.unbind_catalog_fields(&label_of);
        check(&document, &expected["unbound"], name);
        assert_eq!(
            document.to_envelope().expanded_state(),
            expected["unboundState"],
            "{name}: unbound state"
        );
        compared += 1;
    }
    let empty = Document::empty();
    assert_eq!(empty.to_envelope().to_value(), truth["empty"]["envelope"]);
    check(&empty, &truth["empty"]["projections"], "empty");
    let upgraded = Envelope::from_value(truth["v1"]["input"].clone()).unwrap();
    assert_eq!(upgraded.to_value(), truth["v1"]["upgraded"]);
    let upgraded = Document::from_envelope(&upgraded).unwrap();
    check(&upgraded, &truth["v1"]["projections"], "v1");
    let mut dates = 0;
    for pair in truth["dates"].as_array().unwrap() {
        let input = pair[0].as_str().unwrap();
        assert_eq!(
            format_date(input),
            pair[1].as_str().unwrap(),
            "date {input:?}"
        );
        dates += 1;
    }
    eprintln!("compared {compared} templates and {dates} dates with the TypeScript app");
}
