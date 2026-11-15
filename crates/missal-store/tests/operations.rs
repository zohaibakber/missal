mod common;

use common::{Scratch, fir};
use missal_core::document::{Block, Field, Inline, Paragraph, Text};
use missal_core::{
    CustomFieldDraft, DisplayMode, Document, Entity, FieldMarkers, FieldPresentation,
    FieldReference, FirDocumentId, FirId, GlobalPlaceholderDraft, Invalid, PlaceholderId,
    RepositoryError, Revision, TemplateId,
};
use missal_store::{Store, TemplateBundle};

fn empty_bundle() -> TemplateBundle {
    TemplateBundle::decode(&TemplateBundle::encode(1, &[], &[], &[])).unwrap()
}

fn store(scratch: &Scratch) -> Store {
    Store::open_with(scratch.path("test.sqlite"), &empty_bundle()).unwrap()
}

fn document(text: &str, reference: Option<FieldReference>) -> Document {
    let mut children = vec![Inline::Text(Text::new(text))];
    if let Some(reference) = reference {
        children.push(Inline::Field(Field::new(reference)));
    }
    Document::new(vec![Block::Paragraph(Paragraph::new(children))])
}

fn not_found(entity: Entity, id: impl ToString) -> RepositoryError {
    RepositoryError::NotFound {
        entity,
        id: id.to_string(),
    }
}

#[test]
fn firs_are_validated_trimmed_and_round_trip() {
    let scratch = Scratch::new();
    let mut store = store(&scratch);
    assert!(store.firs().unwrap().is_empty());
    let mut draft = fir(" 12/26 ");
    draft.witness = vec![" گواہ ".into()];
    let created = store.create_fir(&draft).unwrap();
    assert_eq!(created.fir.fir_no, "12/26");
    assert_eq!(created.fir.witness, ["گواہ"]);
    assert_eq!(store.fir(created.id).unwrap(), created);
    let missing = FirId::new(99).unwrap();
    assert_eq!(store.fir(missing), Err(not_found(Entity::Fir, missing)));
    let mut invalid = fir("1/26");
    invalid.accused.clear();
    assert_eq!(
        store.create_fir(&invalid),
        Err(RepositoryError::Invalid {
            entity: Entity::Fir,
            reason: Invalid::NoAccused
        })
    );
    let mut updated = created.clone();
    updated.fir.writer = "محرر".into();
    assert_eq!(store.update_fir(&updated).unwrap().fir.writer, "محرر");
    let second = store.create_fir(&fir("13/26")).unwrap();
    let list = store.firs().unwrap();
    assert_eq!(
        list.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        [second.id, created.id]
    );
    updated.id = missing;
    assert_eq!(
        store.update_fir(&updated),
        Err(not_found(Entity::Fir, missing))
    );
}

#[test]
fn fir_summaries_carry_every_accused_name_in_order() {
    let scratch = Scratch::new();
    let mut store = store(&scratch);
    let mut draft = fir("5/26");
    draft.accused = vec![" زاہد حسین ".into(), "طارق جاوید".into()];
    let created = store.create_fir(&draft).unwrap();
    let summary = store.firs().unwrap().remove(0);
    assert_eq!(summary.id, created.id);
    assert_eq!(summary.accused, ["زاہد حسین", "طارق جاوید"]);
}

#[test]
fn template_saves_are_revision_checked() {
    let scratch = Scratch::new();
    let mut store = store(&scratch);
    let summary = store
        .create_template("  پہلا  ", &document("متن", None))
        .unwrap();
    assert_eq!(summary.name, "پہلا");
    assert_eq!(summary.revision, Revision::FIRST);
    assert_eq!(summary.preview_text, "متن");
    let ack = store
        .save_template(
            summary.id,
            Revision::FIRST,
            "دوسرا",
            &document("نیا متن", None),
        )
        .unwrap();
    assert_eq!(ack.revision.get(), 2);
    assert_eq!(ack.preview_text, "نیا متن");
    let stale = store.save_template(summary.id, Revision::FIRST, "x", &Document::empty());
    assert_eq!(
        stale,
        Err(RepositoryError::conflict(Entity::Template, "revision", 1))
    );
    assert_eq!(
        stale.unwrap_err().message(),
        "This was saved elsewhere. Your draft is still here."
    );
    let missing = TemplateId::new(99).unwrap();
    assert_eq!(
        store.save_template(missing, Revision::FIRST, "x", &Document::empty()),
        Err(not_found(Entity::Template, 99))
    );
    let record = store.template(summary.id).unwrap();
    assert_eq!(record.name, "دوسرا");
    assert_eq!(record.document.projections().plain_text, "نیا متن");
    assert!(matches!(
        store.create_template(" ", &Document::empty()),
        Err(RepositoryError::Invalid { .. })
    ));
}

#[test]
fn template_search_matches_name_and_text() {
    let scratch = Scratch::new();
    let mut store = store(&scratch);
    store
        .create_template("چالان", &document("ملزم گرفتار", None))
        .unwrap();
    store
        .create_template("ضمنی", &document("Bank cheque", None))
        .unwrap();
    assert_eq!(store.search_templates("گرفتار").unwrap().len(), 1);
    assert_eq!(store.search_templates("bank").unwrap()[0].name, "ضمنی");
    assert_eq!(store.search_templates(" %_ ").unwrap().len(), 2);
    assert_eq!(store.search_templates("").unwrap().len(), 2);
    assert!(store.search_templates("نہیں").unwrap().is_empty());
}

#[test]
fn templates_in_use_cannot_be_deleted_and_firs_cascade() {
    let scratch = Scratch::new();
    let mut store = store(&scratch);
    let template = store.create_template("چالان", &document("x", None)).unwrap();
    let first = store.create_fir(&fir("1/26")).unwrap();
    let second = store.create_fir(&fir("2/26")).unwrap();
    store
        .add_templates_to_fir(first.id, &[template.id])
        .unwrap();
    store
        .add_templates_to_fir(second.id, &[template.id])
        .unwrap();
    let error = store.remove_template(template.id).unwrap_err();
    assert_eq!(
        error,
        RepositoryError::InUse {
            entity: Entity::Template,
            id: template.id.to_string(),
            used_by: Entity::FirDocument,
            count: 2
        }
    );
    assert_eq!(error.message(), "Template is in use and cannot be deleted");
    let placeholder = store.create_placeholder("بینک").unwrap();
    store
        .set_placeholder_values(first.id, &[(placeholder.id, "HBL".into())])
        .unwrap();
    store.remove_fir(first.id).unwrap();
    assert_eq!(store.fir_documents(first.id).unwrap().len(), 0);
    assert!(store.placeholder_values(first.id).unwrap().is_empty());
    assert_eq!(
        store.remove_fir(first.id),
        Err(not_found(Entity::Fir, first.id))
    );
    let remaining = store.fir_documents(second.id).unwrap();
    store.remove_fir_document(remaining[0].id).unwrap();
    store.remove_template(template.id).unwrap();
    assert_eq!(
        store.remove_template(template.id),
        Err(not_found(Entity::Template, template.id))
    );
}

#[test]
fn removing_several_firs_is_all_or_nothing() {
    let scratch = Scratch::new();
    let mut store = store(&scratch);
    let first = store.create_fir(&fir("1/26")).unwrap();
    let second = store.create_fir(&fir("2/26")).unwrap();
    let missing = FirId::new(50).unwrap();
    assert_eq!(
        store.remove_firs(&[first.id, missing, second.id]),
        Err(not_found(Entity::Fir, missing))
    );
    assert_eq!(store.firs().unwrap().len(), 2);
    store.remove_firs(&[first.id, second.id, first.id]).unwrap();
    assert!(store.firs().unwrap().is_empty());
}

#[test]
fn adding_templates_deduplicates_and_positions() {
    let scratch = Scratch::new();
    let mut store = store(&scratch);
    let fir_record = store.create_fir(&fir("1/26")).unwrap();
    let templates: Vec<TemplateId> = ["الف", "ب", "ج"]
        .iter()
        .map(|name| {
            store
                .create_template(name, &document(name, None))
                .unwrap()
                .id
        })
        .collect();
    let added = store
        .add_templates_to_fir(fir_record.id, &[templates[1], templates[1], templates[0]])
        .unwrap();
    assert_eq!(
        added
            .iter()
            .map(|doc| (doc.template_id, doc.position))
            .collect::<Vec<_>>(),
        [(templates[1], 0), (templates[0], 1)]
    );
    assert_eq!(added[0].title, "ب");
    assert_eq!(added[0].revision, Revision::FIRST);
    let missing = TemplateId::new(99).unwrap();
    assert_eq!(
        store.add_templates_to_fir(fir_record.id, &[templates[2], missing]),
        Err(not_found(Entity::Template, missing))
    );
    assert_eq!(
        store.fir_documents(fir_record.id).unwrap().len(),
        2,
        "a failed add changes nothing"
    );
    let all = store
        .add_templates_to_fir(fir_record.id, &[templates[0], templates[2]])
        .unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[2].template_id, templates[2]);
    assert_eq!(all[2].position, 2);
    let missing_fir = FirId::new(77).unwrap();
    assert_eq!(
        store.add_templates_to_fir(missing_fir, &[templates[0]]),
        Err(not_found(Entity::Fir, missing_fir))
    );
    let saved = store
        .save_template(templates[0], Revision::FIRST, "الف", &document("بدلا", None))
        .unwrap();
    let copy = store.fir_document(all[1].id).unwrap();
    assert_eq!(
        copy.document.projections().plain_text,
        "الف",
        "copies do not follow template edits"
    );
    assert_eq!(copy.source_template_revision, Revision::FIRST);
    assert_eq!(saved.revision.get(), 2);
}

#[test]
fn fir_documents_save_reorder_and_fetch() {
    let scratch = Scratch::new();
    let mut store = store(&scratch);
    let fir_record = store.create_fir(&fir("1/26")).unwrap();
    let other = store.create_fir(&fir("2/26")).unwrap();
    let ids: Vec<TemplateId> = ["الف", "ب", "ج"]
        .iter()
        .map(|name| {
            store
                .create_template(name, &document(name, None))
                .unwrap()
                .id
        })
        .collect();
    let documents = store.add_templates_to_fir(fir_record.id, &ids).unwrap();
    let foreign = store.add_templates_to_fir(other.id, &ids[..1]).unwrap()[0].id;
    let document_ids: Vec<FirDocumentId> = documents.iter().map(|doc| doc.id).collect();
    let reference = FieldReference::Fir(missal_core::FirField {
        property: missal_core::FirProperty::Offence,
        index: None,
    });
    let ack = store
        .save_fir_document(
            document_ids[0],
            Revision::FIRST,
            &document("جرم", Some(reference.clone())),
        )
        .unwrap();
    assert_eq!(ack.field_count, 1);
    assert_eq!(
        store.save_fir_document(document_ids[0], Revision::FIRST, &Document::empty()),
        Err(RepositoryError::conflict(
            Entity::FirDocument,
            "revision",
            1
        ))
    );
    let missing = FirDocumentId::new(999).unwrap();
    assert_eq!(
        store.save_fir_document(missing, Revision::FIRST, &Document::empty()),
        Err(not_found(Entity::FirDocument, missing))
    );
    let reversed: Vec<FirDocumentId> = document_ids.iter().rev().copied().collect();
    let reordered = store
        .reorder_fir_documents(fir_record.id, &reversed)
        .unwrap();
    assert_eq!(
        reordered.iter().map(|doc| doc.id).collect::<Vec<_>>(),
        reversed
    );
    assert_eq!(
        reordered.iter().map(|doc| doc.position).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    let order_conflict = RepositoryError::conflict(Entity::FirDocument, "order", fir_record.id);
    assert_eq!(
        store.reorder_fir_documents(fir_record.id, &reversed[..2]),
        Err(order_conflict.clone())
    );
    assert_eq!(
        store.reorder_fir_documents(fir_record.id, &[reversed[0], reversed[0], reversed[1]]),
        Err(order_conflict)
    );
    assert_eq!(
        store.reorder_fir_documents(fir_record.id, &[reversed[0], reversed[1], foreign]),
        Err(not_found(Entity::FirDocument, foreign))
    );
    let fetched = store
        .fir_documents_by_id(&[document_ids[2], document_ids[0]])
        .unwrap();
    assert_eq!(fetched[0].id, document_ids[2]);
    assert_eq!(fetched[1].document.field_references(), [reference]);
    assert_eq!(fetched[1].revision.get(), 2);
    assert!(store.fir_documents_by_id(&[]).unwrap().is_empty());
    assert_eq!(
        store.fir_documents_by_id(&[document_ids[0], missing]),
        Err(not_found(Entity::FirDocument, missing))
    );
    assert_eq!(
        store.remove_fir_document(missing),
        Err(not_found(Entity::FirDocument, missing))
    );
}

#[test]
fn placeholders_global_values_and_custom_fields() {
    let scratch = Scratch::new();
    let mut store = store(&scratch);
    let bank = store.create_placeholder("  بینک   نام ").unwrap();
    assert_eq!(bank.label, "بینک نام");
    assert_eq!(
        store.create_placeholder("بینک نام"),
        Err(RepositoryError::conflict(
            Entity::Placeholder,
            "name",
            "بینک نام"
        ))
    );
    assert_eq!(
        store.create_placeholder("Date_FIR"),
        Err(RepositoryError::Invalid {
            entity: Entity::Placeholder,
            reason: Invalid::FirFieldName
        })
    );
    let globals = store
        .save_global_placeholders(&[
            GlobalPlaceholderDraft::New {
                label: "تھانہ نام".into(),
                value: "صدر".into(),
            },
            GlobalPlaceholderDraft::New {
                label: "ضلع نام".into(),
                value: "".into(),
            },
        ])
        .unwrap();
    assert_eq!(globals.len(), 2);
    assert_eq!(globals[0].value, "صدر");
    let settings = store.settings().unwrap();
    assert_eq!(
        settings.shared_values.into_iter().collect::<Vec<_>>(),
        [(globals[0].id, "صدر".to_owned())]
    );
    let renamed = store
        .save_global_placeholders(&[GlobalPlaceholderDraft::Existing {
            id: globals[0].id,
            label: "تھانہ".into(),
            value: "سٹی".into(),
        }])
        .unwrap();
    assert_eq!(
        (renamed[0].label.as_str(), renamed[0].value.as_str()),
        ("تھانہ", "سٹی")
    );
    assert_eq!(
        store.save_global_placeholders(&[GlobalPlaceholderDraft::Existing {
            id: bank.id,
            label: "x".into(),
            value: "y".into(),
        }]),
        Err(RepositoryError::storage(
            "globalPlaceholder.save",
            "This field belongs to an FIR, not global values."
        ))
    );
    let before = store.placeholders().unwrap();
    assert_eq!(
        store.save_custom_fields(&[
            CustomFieldDraft::New {
                label: "چیک نمبر".into()
            },
            CustomFieldDraft::Rename {
                id: bank.id,
                label: "تھانہ".into()
            },
        ]),
        Err(RepositoryError::conflict(
            Entity::Placeholder,
            "name",
            "تھانہ"
        ))
    );
    assert_eq!(
        store.placeholders().unwrap(),
        before,
        "a failed save changes nothing"
    );
    let saved = store
        .save_custom_fields(&[
            CustomFieldDraft::Rename {
                id: bank.id,
                label: "بینک".into(),
            },
            CustomFieldDraft::New {
                label: "چیک نمبر".into(),
            },
        ])
        .unwrap();
    assert_eq!(saved.len(), 4);
    assert!(saved.iter().any(|placeholder| placeholder.label == "بینک"));
    assert!(matches!(
        store.save_custom_fields(&[CustomFieldDraft::Rename {
            id: globals[1].id,
            label: "x".into()
        }]),
        Err(RepositoryError::Storage { .. })
    ));
    let missing = PlaceholderId::new(500).unwrap();
    assert_eq!(
        store.rename_placeholder(missing, "x"),
        Err(not_found(Entity::Placeholder, missing))
    );
    assert_eq!(
        store.remove_placeholder(missing),
        Err(not_found(Entity::Placeholder, missing))
    );
    store.remove_placeholder(bank.id).unwrap();
}

#[test]
fn placeholder_values_and_value_context() {
    let scratch = Scratch::new();
    let mut store = store(&scratch);
    let fir_record = store.create_fir(&fir("1/26")).unwrap();
    let bank = store.create_placeholder("بینک").unwrap();
    let station = store
        .save_global_placeholders(&[GlobalPlaceholderDraft::New {
            label: "تھانہ نام".into(),
            value: "صدر".into(),
        }])
        .unwrap()[0]
        .clone();
    let value = |store: &mut Store, fir_id, id, text: &str| {
        store.set_placeholder_values(fir_id, &[(id, text.to_owned())])
    };
    assert_eq!(
        value(&mut store, fir_record.id, bank.id, "HBL").unwrap()[0].value,
        "HBL"
    );
    let updated = value(&mut store, fir_record.id, bank.id, " MCB ").unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].value, "MCB");
    let missing_fir = FirId::new(40).unwrap();
    let missing_placeholder = PlaceholderId::new(40).unwrap();
    assert_eq!(
        value(&mut store, missing_fir, bank.id, "x"),
        Err(not_found(Entity::Fir, missing_fir))
    );
    assert_eq!(
        value(&mut store, fir_record.id, missing_placeholder, "x"),
        Err(not_found(Entity::Placeholder, missing_placeholder))
    );
    assert_eq!(
        store.set_placeholder_values(
            fir_record.id,
            &[(bank.id, "UBL".into()), (missing_placeholder, "x".into())]
        ),
        Err(not_found(Entity::Placeholder, missing_placeholder))
    );
    assert_eq!(
        store.placeholder_values(fir_record.id).unwrap()[0].value,
        "MCB"
    );
    let context = store.fir_value_context(fir_record.id).unwrap();
    assert_eq!(context.catalog.len(), 2);
    let catalog = store.field_catalog().unwrap();
    let workspace = store.fir_workspace(fir_record.id).unwrap();
    let presentation = FieldPresentation::for_fir(&catalog, &workspace.values, DisplayMode::Values);
    assert_eq!(
        presentation
            .display_text(&FieldReference::Catalog(bank.id))
            .text,
        "MCB"
    );
    assert_eq!(
        presentation
            .display_text(&FieldReference::Catalog(station.id))
            .text,
        "صدر"
    );
    assert_eq!(
        store.fir_values(missing_fir),
        Err(not_found(Entity::Fir, missing_fir))
    );
    store.set_shared_value(station.id, "سٹی").unwrap();
    assert!(matches!(
        store.set_shared_value(bank.id, "x"),
        Err(RepositoryError::Storage { .. })
    ));
    assert_eq!(
        store.fir_values(fir_record.id).unwrap().shared_values[&station.id],
        "سٹی"
    );
    assert!(
        value(&mut store, fir_record.id, bank.id, "")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn settings_round_trip() {
    let scratch = Scratch::new();
    let mut store = store(&scratch);
    let defaults = store.settings().unwrap();
    assert_eq!(defaults.field_markers, FieldMarkers::default());
    let markers = FieldMarkers::new("{{", "}}").unwrap();
    assert_eq!(
        store.save_field_markers(&markers).unwrap().field_markers,
        markers
    );
    drop(store);
    let reopened = Store::open_with(scratch.path("test.sqlite"), &empty_bundle()).unwrap();
    assert_eq!(reopened.settings().unwrap().field_markers, markers);
}

#[test]
fn bundled_templates_install_once_with_bound_fields() {
    let scratch = Scratch::new();
    let path = scratch.path("bundle.db");
    let store = Store::open(&path).unwrap();
    let templates = store.templates().unwrap();
    assert_eq!(templates.len(), 36);
    let mut unresolved = Vec::new();
    let mut catalog_fields = 0;
    for summary in &templates {
        for reference in store
            .template(summary.id)
            .unwrap()
            .document
            .field_references()
        {
            match reference {
                FieldReference::Unresolved(label) => unresolved.push(label),
                FieldReference::Catalog(_) => catalog_fields += 1,
                FieldReference::Fir(_) => {}
            }
        }
    }
    assert!(unresolved.is_empty(), "{unresolved:?}");
    assert!(catalog_fields > 0);
    assert_eq!(store.global_placeholders().unwrap().len(), 5);
    drop(store);
    let reopened = Store::open(&path).unwrap();
    assert_eq!(reopened.templates().unwrap().len(), 36);
}

#[test]
fn foreign_and_newer_databases_are_refused() {
    let scratch = Scratch::new();
    let foreign = scratch.path("foreign.db");
    rusqlite::Connection::open(&foreign)
        .unwrap()
        .execute_batch("CREATE TABLE fir_records (id INTEGER PRIMARY KEY)")
        .unwrap();
    assert!(Store::open_with(&foreign, &empty_bundle()).is_err());
    let newer = scratch.path("newer.db");
    drop(store_at(&newer));
    rusqlite::Connection::open(&newer)
        .unwrap()
        .execute_batch("PRAGMA user_version = 99")
        .unwrap();
    assert!(Store::open_with(&newer, &empty_bundle()).is_err());
}

fn store_at(path: &std::path::Path) -> Store {
    Store::open_with(path, &empty_bundle()).unwrap()
}
