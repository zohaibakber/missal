use std::path::{Path, PathBuf};

use missal_core::{FieldCatalog, FieldMarkers, Placeholder, PlaceholderId, PlaceholderSource};
use missal_store::TemplateBundle;
use serde_json::Value;

const VERSION: u32 = 1;

fn labels(fields: &Value, key: &str) -> Vec<String> {
    fields[key]
        .as_array()
        .unwrap_or_else(|| panic!("fields.json has no {key} list"))
        .iter()
        .map(|label| {
            label
                .as_str()
                .expect("labels are strings")
                .trim()
                .to_owned()
        })
        .collect()
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("templates"));
    let output = std::env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("crates/missal-store/bundle/templates.bin"));
    let fields: Value =
        serde_json::from_str(&std::fs::read_to_string(source.join("fields.json")).unwrap())
            .unwrap();
    let (global, custom) = (labels(&fields, "global"), labels(&fields, "custom"));
    let placeholders: Vec<Placeholder> = global
        .iter()
        .chain(&custom)
        .enumerate()
        .map(|(index, label)| Placeholder {
            id: PlaceholderId::new(index as i64 + 1).unwrap(),
            label: label.clone(),
            source: PlaceholderSource::Custom,
        })
        .collect();
    let catalog = FieldCatalog::new(placeholders);
    let markers = FieldMarkers::default();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&source)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "docx")
        })
        .collect();
    files.sort();
    let templates: Vec<_> = files
        .iter()
        .map(|path| {
            let mut imported =
                missal_docx::import(&std::fs::read(path).unwrap(), &catalog, &markers)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let name = path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .trim()
                .to_owned();
            for notice in &imported.notices {
                eprintln!("{name}: {notice}");
            }
            imported.document.unbind_catalog_fields(&|id| {
                catalog.placeholder(id).map(|field| field.label.clone())
            });
            (name, imported.document)
        })
        .collect();
    let bytes = TemplateBundle::encode(VERSION, &global, &custom, &templates);
    std::fs::write(&output, &bytes).unwrap();
    println!(
        "wrote {} templates, {} bytes, to {}",
        templates.len(),
        bytes.len(),
        output.display()
    );
}
