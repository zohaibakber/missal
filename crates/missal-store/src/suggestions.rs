use missal_core::{FirProperty, PlaceholderId, SuggestionEntry, SuggestionKey};

use crate::firs::split;
use crate::{Result, Store, failed};

const FIR_QUERY: &str = "SELECT id, complainant, offence, accused, witness, investigation_officer, writer, recovery FROM firs";
const FIR_COLUMNS: [(FirProperty, bool); 7] = [
    (FirProperty::Complainant, false),
    (FirProperty::Offence, false),
    (FirProperty::Accused, true),
    (FirProperty::Witness, true),
    (FirProperty::InvestigationOfficer, false),
    (FirProperty::Writer, false),
    (FirProperty::Recovery, false),
];
const CUSTOM_QUERY: &str = "SELECT v.fir_id, v.placeholder_id, v.value FROM fir_values v JOIN placeholders p ON p.id = v.placeholder_id WHERE p.shared = 0 AND v.value <> ''";

impl Store {
    pub fn suggestion_entries(&self) -> Result<Vec<SuggestionEntry>> {
        let operation = "suggestions.list";
        let mut entries = Vec::new();
        let mut statement = self
            .connection
            .prepare_cached(FIR_QUERY)
            .map_err(failed(operation))?;
        let mut rows = statement.query([]).map_err(failed(operation))?;
        while let Some(row) = rows.next().map_err(failed(operation))? {
            let stamp: i64 = row.get(0).map_err(failed(operation))?;
            let stamp = stamp.max(0) as u64;
            for (index, (property, list)) in FIR_COLUMNS.into_iter().enumerate() {
                let text = row
                    .get_ref(index + 1)
                    .and_then(|value| Ok(value.as_str()?))
                    .map_err(failed(operation))?;
                let key = SuggestionKey::Fir(property);
                if list {
                    entries.extend(
                        split(text)
                            .into_iter()
                            .map(|value| SuggestionEntry::new(key, value, stamp)),
                    );
                } else if !text.is_empty() {
                    entries.push(SuggestionEntry::new(key, text, stamp));
                }
            }
        }
        let mut statement = self
            .connection
            .prepare_cached(CUSTOM_QUERY)
            .map_err(failed(operation))?;
        let mut rows = statement.query([]).map_err(failed(operation))?;
        while let Some(row) = rows.next().map_err(failed(operation))? {
            let stamp: i64 = row.get(0).map_err(failed(operation))?;
            let placeholder: i64 = row.get(1).map_err(failed(operation))?;
            let Some(placeholder) = PlaceholderId::new(placeholder) else {
                continue;
            };
            let value: String = row.get(2).map_err(failed(operation))?;
            entries.push(SuggestionEntry::new(
                SuggestionKey::Placeholder(placeholder),
                value,
                stamp.max(0) as u64,
            ));
        }
        Ok(entries)
    }
}
