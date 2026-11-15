use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::sync::LazyLock;

use unicode_general_category::{GeneralCategory, get_general_category};

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

pub const FIR_FIELDS: [FirFieldLabel; 27] = [
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
    fir_field("تفتیشی", FirProperty::InvestigationOfficer, None),
    fir_field("تحریر کنندہ", FirProperty::Writer, None),
    fir_field("برآمدگی", FirProperty::Recovery, None),
    fir_field("متن", FirProperty::Body, None),
    fir_field("ایمی نمبر 1", FirProperty::Imei1, None),
    fir_field("ایمی نمبر 2", FirProperty::Imei2, None),
    fir_field("سم نمبر 1", FirProperty::Sim1, None),
    fir_field("سم نمبر 2", FirProperty::Sim2, None),
];

pub fn fir_field_label(field: FirField) -> &'static str {
    FIR_FIELDS
        .iter()
        .find(|candidate| candidate.field == field)
        .map_or(field.property.as_str(), |candidate| candidate.label)
}

static FIR_FIELDS_BY_KEY: LazyLock<HashMap<String, FirField>> = LazyLock::new(|| {
    FIR_FIELDS
        .iter()
        .map(|field| (field_lookup_key(field.label), field.field))
        .collect()
});

pub fn fir_field_by_key(key: &str) -> Option<FirField> {
    FIR_FIELDS_BY_KEY.get(key).copied()
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
        self.by_key.get(&key).cloned()
    }

    pub fn reference_for(&self, name: &str) -> FieldReference {
        self.resolve(name)
            .unwrap_or_else(|| FieldReference::Unresolved(normalize_placeholder_name(name)))
    }

    pub fn label(&self, reference: &FieldReference) -> Option<String> {
        match reference {
            FieldReference::Unresolved(text) => Some(text.clone()),
            FieldReference::Fir(field) => Some(fir_field_label(*field).to_owned()),
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
}

impl<'a> FieldPresentation<'a> {
    pub fn labels(catalog: &'a FieldCatalog) -> FieldPresentation<'a> {
        FieldPresentation {
            catalog,
            values: None,
            mode: DisplayMode::Labels,
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
        }
    }

    pub fn display_text(&self, reference: &FieldReference) -> FieldDisplayText {
        match reference {
            FieldReference::Unresolved(text) => FieldDisplayText {
                text: text.clone(),
                unresolved: true,
            },
            FieldReference::Fir(field) => self.present(fir_field_label(*field), |values| {
                fir_value(&values.fir.fir, *field)
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

pub fn fir_value(fir: &Fir, field: FirField) -> String {
    match fir.value(field.property) {
        FirValue::Text(text) => text.to_owned(),
        FirValue::List(items) => match field.index {
            None => items.join("\n"),
            Some(index) => items.get(index as usize - 1).cloned().unwrap_or_default(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fir::FirStatus;

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

    fn values() -> FirValues {
        FirValues {
            fir: FirRecord {
                id: FirId::new(7).unwrap(),
                fir: Fir {
                    fir_no: "12/26".into(),
                    accused: vec!["علی".into(), "احمد".into()],
                    status: FirStatus::Open,
                    ..Fir::default()
                },
            },
            overrides: vec![PlaceholderValue {
                fir_id: FirId::new(7).unwrap(),
                placeholder_id: PlaceholderId::new(2).unwrap(),
                value: "HBL".into(),
            }],
            shared_values: BTreeMap::from([(PlaceholderId::new(1).unwrap(), "صدر".to_owned())]),
        }
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
    fn display_text_in_both_modes() {
        let catalog = catalog();
        let values = values();
        let accused_2 = FieldReference::Fir(FirField {
            property: FirProperty::Accused,
            index: Some(2),
        });
        let accused = FieldReference::Fir(FirField {
            property: FirProperty::Accused,
            index: None,
        });
        let missing = FieldReference::Fir(FirField {
            property: FirProperty::Writer,
            index: None,
        });
        let shared = FieldReference::Catalog(PlaceholderId::new(1).unwrap());
        let custom = FieldReference::Catalog(PlaceholderId::new(2).unwrap());
        let unknown = FieldReference::Catalog(PlaceholderId::new(9).unwrap());
        let presentation = FieldPresentation::for_fir(&catalog, &values, DisplayMode::Values);
        let text = |reference: &FieldReference| presentation.display_text(reference);
        assert_eq!(text(&accused_2).text, "احمد");
        assert_eq!(text(&accused).text, "علی\nاحمد");
        assert_eq!(
            text(&missing),
            FieldDisplayText {
                text: "تحریر کنندہ".into(),
                unresolved: true
            }
        );
        assert_eq!(text(&shared).text, "صدر");
        assert_eq!(text(&custom).text, "HBL");
        assert_eq!(
            text(&unknown),
            FieldDisplayText {
                text: "9".into(),
                unresolved: true
            }
        );
        let labels = FieldPresentation::for_fir(&catalog, &values, DisplayMode::Labels);
        assert_eq!(
            labels.display_text(&accused_2),
            FieldDisplayText {
                text: "نام ملزم و سکونت 2".into(),
                unresolved: false
            }
        );
        assert_eq!(
            FieldPresentation::labels(&catalog)
                .display_text(&shared)
                .text,
            "تھانہ نام"
        );
        assert!(
            labels
                .display_text(&FieldReference::Unresolved("x".into()))
                .unresolved
        );
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
    fn markers_are_symbols_only() {
        assert!(FieldMarkers::new("«", "»").is_ok());
        assert!(FieldMarkers::new("{{", "}}").is_ok());
        assert!(FieldMarkers::new("<<<", ">>>").is_ok());
        assert!(FieldMarkers::new("<<<<", ">").is_err());
        assert!(FieldMarkers::new("a", "»").is_err());
        assert!(FieldMarkers::new("ن", "»").is_err());
        assert!(FieldMarkers::new("1", "»").is_err());
        assert!(FieldMarkers::new(" ", "»").is_err());
        assert!(FieldMarkers::new("", "»").is_err());
        assert!(FieldMarkers::new("😀", "»").is_ok());
    }
}
