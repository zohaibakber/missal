use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::sync::LazyLock;

use unicode_general_category::{GeneralCategory, get_general_category};

use crate::date::digit_value;
use crate::document::Document;
use crate::error::Invalid;
use crate::fir::{Fir, FirProperty, FirRecord, FirValue};
use crate::ids::{FirId, PlaceholderId};
use crate::text::collapse_whitespace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FirField {
    pub property: FirProperty,
    pub index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FieldReference {
    Catalog(PlaceholderId),
    Fir(FirField),
    Unresolved(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirFieldLabel {
    pub label: &'static str,
    pub field: FirField,
}

const fn fir_field(
    label: &'static str,
    property: FirProperty,
    index: Option<u32>,
) -> FirFieldLabel {
    FirFieldLabel {
        label,
        field: FirField { property, index },
    }
}

pub const FIR_FIELDS: [FirFieldLabel; 41] = [
    fir_field("مقدمہ نمبر", FirProperty::FirNo, None),
    fir_field("مدعی مقدمہ", FirProperty::Complainant, None),
    fir_field("Date FIR", FirProperty::Date, None),
    fir_field("تاریخ ووقت وقوعہ", FirProperty::IncidentDate, None),
    fir_field("تاریخ گرفتاری", FirProperty::ArrestDate, None),
    fir_field("جرم", FirProperty::Offence, None),
    fir_field("نام ملزم و سکونت", FirProperty::Accused, None),
    fir_field("نام ملزم و سکونت 1", FirProperty::Accused, Some(1)),
    fir_field("نام ملزم و سکونت 2", FirProperty::Accused, Some(2)),
    fir_field("گواہان", FirProperty::Witness, None),
    fir_field("گواہان 1", FirProperty::Witness, Some(1)),
    fir_field("گواہان 2", FirProperty::Witness, Some(2)),
    fir_field("ضمنی", FirProperty::Zimni, None),
    fir_field("ضمنی 1", FirProperty::Zimni, Some(1)),
    fir_field("ضمنی 2", FirProperty::Zimni, Some(2)),
    fir_field("ضمنی 3", FirProperty::Zimni, Some(3)),
    fir_field("ضمنی 4", FirProperty::Zimni, Some(4)),
    fir_field("ضمنی 5", FirProperty::Zimni, Some(5)),
    fir_field("ضمنی 6", FirProperty::Zimni, Some(6)),
    fir_field("تاریخ ضمنی 1", FirProperty::ZimniDate, Some(1)),
    fir_field("تاریخ ضمنی 2", FirProperty::ZimniDate, Some(2)),
    fir_field("تاریخ ضمنی 3", FirProperty::ZimniDate, Some(3)),
    fir_field("تاریخ ضمنی 4", FirProperty::ZimniDate, Some(4)),
    fir_field("تاریخ ضمنی 5", FirProperty::ZimniDate, Some(5)),
    fir_field("تاریخ ضمنی 6", FirProperty::ZimniDate, Some(6)),
    fir_field("تاریخ ضمنی", FirProperty::ZimniDate, Some(1)),
    fir_field("ضمنی 1 تاریخ", FirProperty::ZimniDate, Some(1)),
    fir_field("ضمنی 2 تاریخ", FirProperty::ZimniDate, Some(2)),
    fir_field("ضمنی 3 تاریخ", FirProperty::ZimniDate, Some(3)),
    fir_field("ضمنی 4 تاریخ", FirProperty::ZimniDate, Some(4)),
    fir_field("ضمنی 5 تاریخ", FirProperty::ZimniDate, Some(5)),
    fir_field("ضمنی 6 تاریخ", FirProperty::ZimniDate, Some(6)),
    fir_field("تفتیشی", FirProperty::InvestigationOfficer, None),
    fir_field("تحریر کنندہ", FirProperty::Writer, None),
    fir_field("برآمدگی", FirProperty::Recovery, None),
    fir_field("مختصر حالات", FirProperty::Body, None),
    fir_field("متن", FirProperty::Body, None),
    fir_field("ایمی نمبر 1", FirProperty::Imei, Some(1)),
    fir_field("ایمی نمبر 2", FirProperty::Imei, Some(2)),
    fir_field("سم نمبر 1", FirProperty::Sim, Some(1)),
    fir_field("سم نمبر 2", FirProperty::Sim, Some(2)),
];

pub fn fir_field_label(field: FirField) -> Cow<'static, str> {
    if let Some(candidate) = FIR_FIELDS.iter().find(|candidate| candidate.field == field) {
        return Cow::Borrowed(candidate.label);
    }
    match (list_base_label(field.property), field.index) {
        (Some(base), Some(index)) => Cow::Owned(format!("{base} {index}")),
        _ => Cow::Borrowed(field.property.label()),
    }
}

fn list_base_label(property: FirProperty) -> Option<&'static str> {
    match property {
        FirProperty::Accused => Some("نام ملزم و سکونت"),
        FirProperty::Witness => Some("گواہان"),
        FirProperty::Zimni => Some("ضمنی"),
        FirProperty::ZimniDate => Some("تاریخ ضمنی"),
        FirProperty::Imei => Some("ایمی نمبر"),
        FirProperty::Sim => Some("سم نمبر"),
        _ => None,
    }
}

pub const LIST_FIELD_PROPERTIES: [FirProperty; 6] = [
    FirProperty::Accused,
    FirProperty::Witness,
    FirProperty::Zimni,
    FirProperty::ZimniDate,
    FirProperty::Imei,
    FirProperty::Sim,
];

pub fn fir_field_choices<'a>(
    used: impl IntoIterator<Item = &'a FieldReference>,
) -> Vec<(Cow<'static, str>, FirField)> {
    let mut highest: HashMap<FirProperty, u32> = HashMap::new();
    for reference in used {
        if let FieldReference::Fir(FirField {
            property,
            index: Some(index),
        }) = reference
        {
            let entry = highest.entry(*property).or_default();
            *entry = (*entry).max(*index);
        }
    }
    let mut out: Vec<(Cow<'static, str>, FirField)> = Vec::new();
    for (position, field) in FIR_FIELDS.iter().enumerate() {
        if !out.iter().any(|(_, existing)| *existing == field.field) {
            out.push((Cow::Borrowed(field.label), field.field));
        }
        let property = field.field.property;
        let last_of_property = !FIR_FIELDS[position + 1..]
            .iter()
            .any(|next| next.field.property == property && next.field.index.is_some());
        if field.field.index.is_none() || !last_of_property {
            continue;
        }
        let listed = out
            .iter()
            .filter(|(_, existing)| existing.property == property)
            .filter_map(|(_, existing)| existing.index)
            .max()
            .unwrap_or(0);
        let wanted = highest.get(&property).copied().unwrap_or(0).max(listed) + 1;
        for index in listed + 1..=wanted.min(MAX_FIELD_INDEX) {
            let field = FirField {
                property,
                index: Some(index),
            };
            out.push((fir_field_label(field), field));
        }
    }
    out
}

const MAX_FIELD_INDEX: u32 = 999;

fn split_index(key: &str) -> Option<(&str, u32, &str)> {
    let start = key.find(|c: char| digit_value(c).is_some())?;
    let digits_len: usize = key[start..]
        .chars()
        .take_while(|&c| digit_value(c).is_some())
        .map(char::len_utf8)
        .sum();
    let index = key[start..start + digits_len]
        .chars()
        .try_fold(0u32, |total, c| {
            let next = total * 10 + digit_value(c)?;
            (next <= MAX_FIELD_INDEX).then_some(next)
        })?;
    Some((&key[..start], index, &key[start + digits_len..]))
}

fn indexed_field_by_key(key: &str) -> Option<FirField> {
    let (prefix, index, suffix) = split_index(key)?;
    if index == 0 {
        return None;
    }
    let property = LIST_FIELD_PROPERTIES.into_iter().find(|&property| {
        let base = field_lookup_key(list_base_label(property).unwrap_or_default());
        match property {
            FirProperty::ZimniDate => {
                (prefix == base && suffix.is_empty()) || (prefix == "ضمنی" && suffix == "تاریخ")
            }
            _ => prefix == base && suffix.is_empty(),
        }
    })?;
    Some(FirField {
        property,
        index: Some(index),
    })
}

static FIR_FIELDS_BY_KEY: LazyLock<HashMap<String, FirField>> = LazyLock::new(|| {
    FIR_FIELDS
        .iter()
        .map(|field| (field_lookup_key(field.label), field.field))
        .collect()
});

pub fn fir_field_by_key(key: &str) -> Option<FirField> {
    FIR_FIELDS_BY_KEY
        .get(key)
        .copied()
        .or_else(|| indexed_field_by_key(key))
}

pub fn field_lookup_key(name: &str) -> String {
    name.chars()
        .filter(|&c| !(c.is_whitespace() || c == '_' || c == '\u{200C}'))
        .collect()
}

pub fn normalize_placeholder_name(name: &str) -> String {
    collapse_whitespace(name)
}

pub fn placeholder_name(name: &str) -> Result<String, Invalid> {
    let normalized = normalize_placeholder_name(name);
    if normalized.is_empty() {
        return Err(Invalid::EmptyName);
    }
    if fir_field_by_key(&field_lookup_key(&normalized)).is_some() {
        return Err(Invalid::FirFieldName);
    }
    Ok(normalized)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaceholderSource {
    Shared,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placeholder {
    pub id: PlaceholderId,
    pub label: String,
    pub source: PlaceholderSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalPlaceholder {
    pub id: PlaceholderId,
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobalPlaceholderDraft {
    Existing {
        id: PlaceholderId,
        label: String,
        value: String,
    },
    New {
        label: String,
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomFieldDraft {
    Rename { id: PlaceholderId, label: String },
    New { label: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceholderValue {
    pub fir_id: FirId,
    pub placeholder_id: PlaceholderId,
    pub value: String,
}

#[derive(Debug, Clone, Default)]
pub struct FieldCatalog {
    placeholders: Vec<Placeholder>,
    by_id: HashMap<PlaceholderId, usize>,
    by_key: HashMap<String, FieldReference>,
}

impl FieldCatalog {
    pub fn new(placeholders: Vec<Placeholder>) -> FieldCatalog {
        let mut by_key: HashMap<String, FieldReference> = FIR_FIELDS_BY_KEY
            .iter()
            .map(|(key, field)| (key.clone(), FieldReference::Fir(*field)))
            .collect();
        let mut by_id = HashMap::with_capacity(placeholders.len());
        for (position, placeholder) in placeholders.iter().enumerate() {
            by_id.insert(placeholder.id, position);
            by_key.insert(
                field_lookup_key(&placeholder.label),
                FieldReference::Catalog(placeholder.id),
            );
        }
        FieldCatalog {
            placeholders,
            by_id,
            by_key,
        }
    }

    pub fn placeholders(&self) -> &[Placeholder] {
        &self.placeholders
    }

    pub fn placeholder(&self, id: PlaceholderId) -> Option<&Placeholder> {
        self.by_id
            .get(&id)
            .map(|&position| &self.placeholders[position])
    }

    pub fn resolve(&self, name: &str) -> Option<FieldReference> {
        let key = field_lookup_key(name);
        if key.is_empty() {
            return None;
        }
        self.by_key
            .get(&key)
            .cloned()
            .or_else(|| indexed_field_by_key(&key).map(FieldReference::Fir))
    }

    pub fn reference_for(&self, name: &str) -> FieldReference {
        self.resolve(name)
            .unwrap_or_else(|| FieldReference::Unresolved(normalize_placeholder_name(name)))
    }

    pub fn label(&self, reference: &FieldReference) -> Option<String> {
        match reference {
            FieldReference::Unresolved(text) => Some(text.clone()),
            FieldReference::Fir(field) => Some(fir_field_label(*field).into_owned()),
            FieldReference::Catalog(id) => self.placeholder(*id).map(|field| field.label.clone()),
        }
    }

    pub fn find_token(
        &self,
        text: &str,
        markers: &FieldMarkers,
        from: usize,
    ) -> Option<TokenMatch> {
        let mut search = from;
        while let Some(found) = text.get(search..)?.find(markers.open.as_str()) {
            let open = search + found;
            let name_start = open + markers.open.len();
            let close = name_start + text[name_start..].find(markers.close.as_str())?;
            let name = &text[name_start..close];
            if name.chars().count() <= MAX_TOKEN_NAME_LENGTH
                && !name.contains('\n')
                && let Some(reference) = self.resolve(name)
            {
                return Some(TokenMatch {
                    range: open..close + markers.close.len(),
                    reference,
                });
            }
            search = open + text[open..].chars().next().map_or(1, char::len_utf8);
        }
        None
    }
}

pub const MAX_TOKEN_NAME_LENGTH: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenMatch {
    pub range: Range<usize>,
    pub reference: FieldReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldMarkers {
    open: String,
    close: String,
}

impl FieldMarkers {
    pub fn new(open: &str, close: &str) -> Result<FieldMarkers, Invalid> {
        if is_valid_marker(open) && is_valid_marker(close) {
            Ok(FieldMarkers {
                open: open.to_owned(),
                close: close.to_owned(),
            })
        } else {
            Err(Invalid::FieldMarker)
        }
    }

    pub fn open(&self) -> &str {
        &self.open
    }

    pub fn close(&self) -> &str {
        &self.close
    }

    pub fn token(&self, name: &str) -> String {
        format!("{}{name}{}", self.open, self.close)
    }
}

impl Default for FieldMarkers {
    fn default() -> FieldMarkers {
        FieldMarkers {
            open: "«".to_owned(),
            close: "»".to_owned(),
        }
    }
}

pub fn is_valid_marker(marker: &str) -> bool {
    let count = marker.chars().count();
    (1..=3).contains(&count)
        && marker.chars().all(|c| {
            !c.is_whitespace()
                && !matches!(
                    get_general_category(c),
                    GeneralCategory::UppercaseLetter
                        | GeneralCategory::LowercaseLetter
                        | GeneralCategory::TitlecaseLetter
                        | GeneralCategory::ModifierLetter
                        | GeneralCategory::OtherLetter
                        | GeneralCategory::DecimalNumber
                        | GeneralCategory::LetterNumber
                        | GeneralCategory::OtherNumber
                )
        })
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppSettings {
    pub shared_values: BTreeMap<PlaceholderId, String>,
    pub field_markers: FieldMarkers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirValues {
    pub fir: FirRecord,
    pub overrides: Vec<PlaceholderValue>,
    pub shared_values: BTreeMap<PlaceholderId, String>,
}

impl FirValues {
    fn override_value(&self, id: PlaceholderId) -> Option<&str> {
        self.overrides
            .iter()
            .rev()
            .find(|value| value.placeholder_id == id)
            .map(|value| value.value.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirValueContext {
    pub values: FirValues,
    pub catalog: Vec<Placeholder>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayMode {
    Labels,
    #[default]
    Values,
}

impl DisplayMode {
    pub fn toggled(self) -> DisplayMode {
        match self {
            DisplayMode::Labels => DisplayMode::Values,
            DisplayMode::Values => DisplayMode::Labels,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDisplayText {
    pub text: String,
    pub unresolved: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct FieldPresentation<'a> {
    pub catalog: &'a FieldCatalog,
    pub values: Option<&'a FirValues>,
    pub mode: DisplayMode,
    pub tails: ListTails,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ListTails {
    accused: u32,
    witness: u32,
    imei: u32,
    sim: u32,
}

impl ListTails {
    pub fn of(document: &Document) -> ListTails {
        let mut tails = ListTails::default();
        document.for_each_field(&mut |field| {
            if let FieldReference::Fir(FirField {
                property,
                index: Some(index),
            }) = field.reference
                && let Some(tail) = tails.slot(property)
            {
                *tail = (*tail).max(index);
            }
        });
        tails
    }

    fn slot(&mut self, property: FirProperty) -> Option<&mut u32> {
        match property {
            FirProperty::Accused => Some(&mut self.accused),
            FirProperty::Witness => Some(&mut self.witness),
            FirProperty::Imei => Some(&mut self.imei),
            FirProperty::Sim => Some(&mut self.sim),
            _ => None,
        }
    }

    fn takes_rest(mut self, field: FirField) -> bool {
        field.index.is_some_and(|index| {
            index >= 2 && self.slot(field.property).is_some_and(|tail| *tail == index)
        })
    }
}

impl<'a> FieldPresentation<'a> {
    pub fn new(
        catalog: &'a FieldCatalog,
        values: Option<&'a FirValues>,
        mode: DisplayMode,
    ) -> FieldPresentation<'a> {
        match values {
            Some(values) => FieldPresentation::for_fir(catalog, values, mode),
            None => FieldPresentation::labels(catalog),
        }
    }

    pub fn labels(catalog: &'a FieldCatalog) -> FieldPresentation<'a> {
        FieldPresentation {
            catalog,
            values: None,
            mode: DisplayMode::Labels,
            tails: ListTails::default(),
        }
    }

    pub fn for_fir(
        catalog: &'a FieldCatalog,
        values: &'a FirValues,
        mode: DisplayMode,
    ) -> FieldPresentation<'a> {
        FieldPresentation {
            catalog,
            values: Some(values),
            mode,
            tails: ListTails::default(),
        }
    }

    pub fn for_document(self, document: &Document) -> FieldPresentation<'a> {
        FieldPresentation {
            tails: ListTails::of(document),
            ..self
        }
    }

    pub fn display_text(&self, reference: &FieldReference) -> FieldDisplayText {
        match reference {
            FieldReference::Unresolved(text) => FieldDisplayText {
                text: text.clone(),
                unresolved: true,
            },
            FieldReference::Fir(field) => self.present(&fir_field_label(*field), |values| {
                if self.tails.takes_rest(*field) {
                    fir_value_and_rest(&values.fir.fir, *field)
                } else {
                    fir_value(&values.fir.fir, *field)
                }
            }),
            FieldReference::Catalog(id) => match self.catalog.placeholder(*id) {
                Some(placeholder) => {
                    self.present(&placeholder.label, |values| match &placeholder.source {
                        PlaceholderSource::Shared => values
                            .shared_values
                            .get(&placeholder.id)
                            .cloned()
                            .unwrap_or_default(),
                        PlaceholderSource::Custom => values
                            .override_value(placeholder.id)
                            .unwrap_or_default()
                            .to_owned(),
                    })
                }
                None => FieldDisplayText {
                    text: id.to_string(),
                    unresolved: true,
                },
            },
        }
    }

    fn present(&self, label: &str, value: impl FnOnce(&FirValues) -> String) -> FieldDisplayText {
        let Some(values) = self.values.filter(|_| self.mode == DisplayMode::Values) else {
            return FieldDisplayText {
                text: label.to_owned(),
                unresolved: false,
            };
        };
        let text = value(values);
        if text.trim().is_empty() {
            FieldDisplayText {
                text: label.to_owned(),
                unresolved: true,
            }
        } else {
            FieldDisplayText {
                text,
                unresolved: false,
            }
        }
    }
}

pub const LIST_SEPARATOR: &str = "، ";

pub fn fir_value(fir: &Fir, field: FirField) -> String {
    match fir.value(field.property) {
        FirValue::Text(text) => text.to_owned(),
        FirValue::List(items) => match field.index {
            None => join_inline(items.iter().map(String::as_str)),
            Some(index) => items.get(index as usize - 1).cloned().unwrap_or_default(),
        },
        FirValue::Zimni(entries, part) => match field.index {
            None => join_inline(entries.iter().map(|entry| entry.part(part))),
            Some(index) => entries
                .get(index as usize - 1)
                .map_or_default(|entry| entry.part(part).to_owned()),
        },
    }
}

fn fir_value_and_rest(fir: &Fir, field: FirField) -> String {
    match (fir.value(field.property), field.index) {
        (FirValue::List(items), Some(index)) => {
            join_inline(items.iter().skip(index as usize - 1).map(String::as_str))
        }
        _ => fir_value(fir, field),
    }
}

fn join_inline<'a>(items: impl Iterator<Item = &'a str>) -> String {
    items
        .map(|item| item.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>()
        .join(LIST_SEPARATOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> FieldCatalog {
        FieldCatalog::new(vec![
            Placeholder {
                id: PlaceholderId::new(1).unwrap(),
                label: "تھانہ نام".into(),
                source: PlaceholderSource::Shared,
            },
            Placeholder {
                id: PlaceholderId::new(2).unwrap(),
                label: "بینک".into(),
                source: PlaceholderSource::Custom,
            },
        ])
    }

    #[test]
    fn lookup_key_ignores_spacing_underscores_and_zwnj() {
        assert_eq!(field_lookup_key("Date_FIR"), "DateFIR");
        assert_eq!(field_lookup_key("نام\u{200C} ملزم"), "نامملزم");
        assert_eq!(normalize_placeholder_name("  a \t b "), "a b");
        assert_eq!(placeholder_name("Date  FIR"), Err(Invalid::FirFieldName));
        assert_eq!(placeholder_name(" "), Err(Invalid::EmptyName));
    }

    #[test]
    fn tokens_are_found_with_configured_markers() {
        let catalog = catalog();
        let markers = FieldMarkers::default();
        let text = "«نہیں» مقدمہ «مقدمہ  نمبر» اور «بینک»";
        let first = catalog.find_token(text, &markers, 0).unwrap();
        assert_eq!(&text[first.range.clone()], "«مقدمہ  نمبر»");
        assert_eq!(
            first.reference,
            FieldReference::Fir(FirField {
                property: FirProperty::FirNo,
                index: None
            })
        );
        let second = catalog.find_token(text, &markers, first.range.end).unwrap();
        assert_eq!(
            second.reference,
            FieldReference::Catalog(PlaceholderId::new(2).unwrap())
        );
        let braces = FieldMarkers::new("{{", "}}").unwrap();
        assert!(catalog.find_token("{{جرم}}", &braces, 0).is_some());
        assert!(catalog.find_token("«جرم", &markers, 0).is_none());
        assert!(catalog.find_token("«جر\nم»", &markers, 0).is_none());
        assert_eq!(
            catalog.reference_for(" نا معلوم  نام "),
            FieldReference::Unresolved("نا معلوم نام".into())
        );
    }

    #[test]
    fn list_fields_take_any_index_and_join_inline() {
        let catalog = catalog();
        let field = |property, index| FieldReference::Fir(FirField { property, index });
        assert_eq!(
            catalog.resolve("گواہان 7"),
            Some(field(FirProperty::Witness, Some(7)))
        );
        assert_eq!(
            catalog.resolve("گواہان۳"),
            Some(field(FirProperty::Witness, Some(3)))
        );
        assert_eq!(
            catalog.resolve("نام ملزم و سکونت 4"),
            Some(field(FirProperty::Accused, Some(4)))
        );
        assert_eq!(
            catalog.resolve("ضمنی 9 تاریخ"),
            Some(field(FirProperty::ZimniDate, Some(9)))
        );
        assert_eq!(catalog.resolve("گواہان 0"), None);
        assert_eq!(catalog.resolve("جرم 2"), None);
        assert_eq!(
            catalog
                .label(&field(FirProperty::Witness, Some(7)))
                .as_deref(),
            Some("گواہان 7")
        );
        assert_eq!(placeholder_name("گواہان 12"), Err(Invalid::FirFieldName));

        let fir = Fir {
            accused: vec![
                "منیر احمد سکنہ بھائی گیٹ لاہور".into(),
                " سلیمان ولد ارشد ".into(),
            ],
            witness: (1..=4).map(|n| format!("گواہ {n}")).collect(),
            ..Fir::default()
        };
        assert_eq!(
            fir_value(
                &fir,
                FirField {
                    property: FirProperty::Accused,
                    index: None
                }
            ),
            "منیر احمد سکنہ بھائی گیٹ لاہور، سلیمان ولد ارشد"
        );
        assert_eq!(
            fir_value(
                &fir,
                FirField {
                    property: FirProperty::Witness,
                    index: Some(4)
                }
            ),
            "گواہ 4"
        );

        let used = [field(FirProperty::Witness, Some(5))];
        let witnesses: Vec<String> = fir_field_choices(&used)
            .into_iter()
            .filter(|(_, field)| field.property == FirProperty::Witness)
            .map(|(label, _)| label.into_owned())
            .collect();
        assert_eq!(
            witnesses,
            [
                "گواہان",
                "گواہان 1",
                "گواہان 2",
                "گواہان 3",
                "گواہان 4",
                "گواہان 5",
                "گواہان 6"
            ]
        );
    }

    #[test]
    fn last_numbered_witness_slot_lists_the_rest() {
        use crate::document::{Block, Field, Inline, Paragraph};
        let catalog = catalog();
        let witness = |index| {
            FieldReference::Fir(FirField {
                property: FirProperty::Witness,
                index: Some(index),
            })
        };
        let document = |indices: &[u32]| {
            let mut document = Document::empty();
            document.blocks = indices
                .iter()
                .map(|&index| {
                    Block::Paragraph(Paragraph::new(vec![Inline::Field(Field::new(witness(
                        index,
                    )))]))
                })
                .collect();
            document
        };
        let values = FirValues {
            fir: FirRecord {
                id: FirId::new(1).unwrap(),
                fir: Fir {
                    witness: vec!["محمد اسلم".into(), "طارق محمود".into(), "عمران علی".into()],
                    ..Fir::default()
                },
            },
            overrides: Vec::new(),
            shared_values: BTreeMap::new(),
        };
        let shown = |document: &Document, index| {
            FieldPresentation::for_fir(&catalog, &values, DisplayMode::Values)
                .for_document(document)
                .display_text(&witness(index))
                .text
        };
        let pair = document(&[1, 2]);
        assert_eq!(shown(&pair, 1), "محمد اسلم");
        assert_eq!(shown(&pair, 2), "طارق محمود، عمران علی");
        let three = document(&[1, 2, 3]);
        assert_eq!(shown(&three, 2), "طارق محمود");
        assert_eq!(shown(&three, 3), "عمران علی");
        let single = document(&[1]);
        assert_eq!(shown(&single, 1), "محمد اسلم");
    }

    #[test]
    fn mobile_numbers_are_lists_and_old_documents_keep_their_slots() {
        use crate::document::{Block, Field, Inline, Paragraph};
        let lookup = |label: &str| fir_field_by_key(&field_lookup_key(label));
        let sim = |index| FirField {
            property: FirProperty::Sim,
            index: Some(index),
        };
        assert_eq!(
            lookup("ایمی نمبر 2"),
            Some(FirField {
                property: FirProperty::Imei,
                index: Some(2)
            })
        );
        assert_eq!(lookup("سم نمبر 3"), Some(sim(3)));
        assert_eq!(
            FirProperty::decode(13, None),
            Some((FirProperty::Imei, Some(1)))
        );
        assert_eq!(
            FirProperty::decode(14, None),
            Some((FirProperty::Imei, Some(2)))
        );
        assert_eq!(
            FirProperty::decode(16, None),
            Some((FirProperty::Sim, Some(2)))
        );
        assert_eq!(
            FirProperty::decode(FirProperty::ZimniDate as u8, Some(3)),
            Some((FirProperty::ZimniDate, Some(3)))
        );
        let catalog = catalog();
        let values = FirValues {
            fir: FirRecord {
                id: FirId::new(1).unwrap(),
                fir: Fir {
                    sim: vec![
                        "03001234567".into(),
                        "03111234567".into(),
                        "03221234567".into(),
                    ],
                    ..Fir::default()
                },
            },
            overrides: Vec::new(),
            shared_values: BTreeMap::new(),
        };
        let mut document = Document::empty();
        document.blocks = [1, 2]
            .map(|index| {
                Block::Paragraph(Paragraph::new(vec![Inline::Field(Field::new(
                    FieldReference::Fir(sim(index)),
                ))]))
            })
            .into();
        let shown = |index| {
            FieldPresentation::for_fir(&catalog, &values, DisplayMode::Values)
                .for_document(&document)
                .display_text(&FieldReference::Fir(sim(index)))
                .text
        };
        assert_eq!(shown(1), "03001234567");
        assert_eq!(shown(2), "03111234567، 03221234567");
    }
}
