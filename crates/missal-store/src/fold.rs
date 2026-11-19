use std::collections::HashMap;

use missal_core::{
    FieldReference, FirField, FirId, FirProperty, PlaceholderId, PlaceholderSource, Zimni,
    field_lookup_key, fir_field_by_key,
};
use rusqlite::{Connection, Transaction, params};

use crate::firs::{find_fir, write_zimni};
use crate::{Result, failed, id, pack, placeholders, unpack};

const OPERATION: &str = "database.fold";

const BODIES: [(&str, &str); 4] = [
    ("template_bodies", "template_id"),
    ("document_bodies", "document_id"),
    ("template_drafts", "template_id"),
    ("document_drafts", "document_id"),
];

fn foldable(field: FirField) -> bool {
    !matches!(field.property, FirProperty::Accused | FirProperty::Witness)
        && (field.index.is_some() || !field.property.is_list())
}

pub(crate) fn fold_fir_fields(connection: &mut Connection) -> Result<()> {
    let folds: HashMap<PlaceholderId, FirField> = placeholders::list(connection, OPERATION)?
        .into_iter()
        .filter(|placeholder| placeholder.source == PlaceholderSource::Custom)
        .filter_map(|placeholder| {
            fir_field_by_key(&field_lookup_key(&placeholder.label))
                .filter(|field| foldable(*field))
                .map(|field| (placeholder.id, field))
        })
        .collect();
    if folds.is_empty() {
        return Ok(());
    }
    let transaction = connection.transaction().map_err(failed(OPERATION))?;
    for (placeholder, field) in &folds {
        copy_values(&transaction, *placeholder, *field)?;
    }
    for (table, key) in BODIES {
        rewrite_bodies(&transaction, table, key, &folds)?;
    }
    for placeholder in folds.keys() {
        transaction
            .execute(
                "DELETE FROM placeholders WHERE id = ?1",
                [placeholder.get()],
            )
            .map_err(failed(OPERATION))?;
    }
    transaction.commit().map_err(failed(OPERATION))
}

fn copy_values(
    transaction: &Transaction<'_>,
    placeholder: PlaceholderId,
    field: FirField,
) -> Result<()> {
    let rows: Vec<(i64, String)> = transaction
        .prepare("SELECT fir_id, value FROM fir_values WHERE placeholder_id = ?1")
        .and_then(|mut statement| {
            statement
                .query_map([placeholder.get()], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect()
        })
        .map_err(failed(OPERATION))?;
    for (fir_id, value) in rows {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        let fir_id = id(fir_id, FirId::new, OPERATION)?;
        match (field.property, field.index) {
            (FirProperty::Zimni | FirProperty::ZimniDate, Some(index)) => {
                let mut zimni = find_fir(transaction, fir_id, OPERATION)?.fir.zimni;
                let position = index as usize - 1;
                if zimni.len() <= position {
                    zimni.resize(position + 1, Zimni::default());
                }
                let slot = match field.property {
                    FirProperty::Zimni => &mut zimni[position].number,
                    _ => &mut zimni[position].date,
                };
                if slot.is_empty() {
                    *slot = value.to_owned();
                    write_zimni(transaction, fir_id, &zimni, OPERATION)?;
                }
            }
            (property, _) => {
                transaction
                    .execute(
                        &format!(
                            "UPDATE firs SET {column} = ?1 WHERE id = ?2 AND {column} = ''",
                            column = property.as_str()
                        ),
                        params![value, fir_id.get()],
                    )
                    .map_err(failed(OPERATION))?;
            }
        }
    }
    Ok(())
}

fn rewrite_bodies(
    transaction: &Transaction<'_>,
    table: &str,
    key: &str,
    folds: &HashMap<PlaceholderId, FirField>,
) -> Result<()> {
    let rows: Vec<(i64, Vec<u8>)> = transaction
        .prepare(&format!("SELECT {key}, body FROM {table}"))
        .and_then(|mut statement| {
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect()
        })
        .map_err(failed(OPERATION))?;
    for (row_id, body) in rows {
        let mut document = unpack(&body, OPERATION)?;
        let mut changed = false;
        document.for_each_field_mut(&mut |field| {
            if let FieldReference::Catalog(placeholder) = field.reference
                && let Some(fir_field) = folds.get(&placeholder)
            {
                field.reference = FieldReference::Fir(*fir_field);
                changed = true;
            }
        });
        if changed {
            transaction
                .execute(
                    &format!("UPDATE {table} SET body = ?1 WHERE {key} = ?2"),
                    params![pack(&document), row_id],
                )
                .map_err(failed(OPERATION))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use missal_core::document::{Block, Field, Inline, Paragraph};
    use missal_core::{Document, Fir};

    use super::*;
    use crate::Store;

    fn field_document(references: Vec<FieldReference>) -> Document {
        Document::new(vec![Block::Paragraph(Paragraph::new(
            references
                .into_iter()
                .map(|reference| Inline::Field(Field::new(reference)))
                .collect(),
        ))])
    }

    #[test]
    fn placeholders_named_like_fir_fields_fold_into_them() {
        let mut store = Store::in_memory().unwrap();
        let fir = store
            .create_fir(&Fir {
                fir_no: "12/26".into(),
                date: "01-09-2026".into(),
                offence: "چوری".into(),
                incident_date: "01-09-2026".into(),
                accused: vec!["علی".into()],
                zimni: vec![Zimni::new("1", "")],
                ..Fir::default()
            })
            .unwrap();
        let summary =
            placeholders::insert(&store.connection, "مختصر حالات", false, "", "t").unwrap();
        let date = placeholders::insert(&store.connection, "تاریخ ضمنی 4", false, "", "t").unwrap();
        let bank = placeholders::insert(&store.connection, "بینک", false, "", "t").unwrap();
        for (placeholder, value) in [(summary, "حالات"), (date, "05-09-2026"), (bank, "HBL")] {
            store
                .connection
                .execute(
                    "INSERT INTO fir_values (fir_id, placeholder_id, value) VALUES (?1, ?2, ?3)",
                    params![fir.id.get(), placeholder.get(), value],
                )
                .unwrap();
        }
        let template = store
            .create_template(
                "ضمنی",
                &field_document(vec![
                    FieldReference::Catalog(summary),
                    FieldReference::Catalog(date),
                    FieldReference::Catalog(bank),
                ]),
            )
            .unwrap();
        fold_fir_fields(&mut store.connection).unwrap();
        let folded = store.fir(fir.id).unwrap().fir;
        assert_eq!(folded.body, "حالات");
        assert_eq!(
            folded.zimni,
            [
                Zimni::new("1", ""),
                Zimni::default(),
                Zimni::default(),
                Zimni::new("", "05-09-2026")
            ]
        );
        let labels: Vec<String> = store
            .placeholders()
            .unwrap()
            .into_iter()
            .map(|placeholder| placeholder.label)
            .collect();
        assert_eq!(labels, ["بینک"]);
        assert_eq!(
            store
                .template(template.id)
                .unwrap()
                .document
                .field_references(),
            [
                FieldReference::Fir(FirField {
                    property: FirProperty::Body,
                    index: None
                }),
                FieldReference::Fir(FirField {
                    property: FirProperty::ZimniDate,
                    index: Some(4)
                }),
                FieldReference::Catalog(bank),
            ]
        );
    }
}
