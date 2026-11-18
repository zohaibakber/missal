use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use missal_core::document::Document;
use missal_core::{
    FieldCatalog, FieldMarkers, FieldReference, Placeholder, PlaceholderId, PlaceholderSource,
};
use serde_json::Value;

use crate::{Notice, import};

#[derive(Debug, Clone)]
pub struct FolderTemplate {
    pub name: String,
    pub document: Document,
    pub global: Vec<String>,
    pub custom: Vec<String>,
    pub notices: Vec<Notice>,
}

#[derive(Debug, Clone)]
pub struct Folder {
    pub global: Vec<String>,
    pub custom: Vec<String>,
    pub templates: Vec<FolderTemplate>,
}

impl Folder {
    pub fn documents(&self) -> Vec<(String, Document)> {
        self.templates
            .iter()
            .map(|template| (template.name.clone(), template.document.clone()))
            .collect()
    }
}

fn labels(fields: &Value, key: &str) -> Result<Vec<String>, String> {
    fields[key]
        .as_array()
        .ok_or_else(|| format!("fields.json has no {key} list"))?
        .iter()
        .map(|label| {
            label
                .as_str()
                .map(|label| label.trim().to_owned())
                .ok_or_else(|| format!("fields.json {key} labels must be strings"))
        })
        .collect()
}

pub fn import_folder(source: &Path) -> Result<Folder, String> {
    let fields = source.join("fields.json");
    let fields: Value = std::fs::read_to_string(&fields)
        .map_err(|error| format!("{}: {error}", fields.display()))
        .and_then(|text| serde_json::from_str(&text).map_err(|error| error.to_string()))?;
    let (global, custom) = (labels(&fields, "global")?, labels(&fields, "custom")?);
    let all: Vec<String> = global.iter().chain(&custom).cloned().collect();
    let placeholders: Vec<Placeholder> = all
        .iter()
        .enumerate()
        .map(|(index, label)| Placeholder {
            id: PlaceholderId::new(index as i64 + 1).expect("placeholder ids start at one"),
            label: label.clone(),
            source: PlaceholderSource::Custom,
        })
        .collect();
    let catalog = FieldCatalog::new(placeholders);
    let markers = FieldMarkers::default();
    let mut files: Vec<PathBuf> = std::fs::read_dir(source)
        .map_err(|error| format!("{}: {error}", source.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "docx")
        })
        .collect();
    files.sort();
    let templates = files
        .iter()
        .map(|path| {
            let bytes =
                std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
            let mut imported = import(&bytes, &catalog, &markers)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            let name = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().trim().to_owned())
                .unwrap_or_default();
            let used: BTreeSet<usize> = imported
                .document
                .field_references()
                .into_iter()
                .filter_map(|reference| match reference {
                    FieldReference::Catalog(id) => Some(id.get() as usize - 1),
                    _ => None,
                })
                .collect();
            imported.document.unbind_catalog_fields(&|id| {
                catalog.placeholder(id).map(|field| field.label.clone())
            });
            let (mut template_global, mut template_custom) = (Vec::new(), Vec::new());
            for index in used {
                if index < global.len() {
                    template_global.push(all[index].clone());
                } else {
                    template_custom.push(all[index].clone());
                }
            }
            Ok(FolderTemplate {
                name,
                document: imported.document,
                global: template_global,
                custom: template_custom,
                notices: imported.notices,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Folder {
        global,
        custom,
        templates,
    })
}

pub fn repository_templates() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates")
}
