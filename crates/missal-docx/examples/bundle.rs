//! Packs templates/ into the bundle the app ships with:
//! `cargo run -p missal-docx --example bundle`.

use std::path::{Path, PathBuf};

use missal_store::TemplateBundle;

const VERSION: u32 = 1;

fn main() {
    let source = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(missal_docx::repository_templates);
    let output = std::env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../missal-store/bundle/templates.bin")
        });
    let folder = missal_docx::import_folder(&source).unwrap_or_else(|error| panic!("{error}"));
    let templates = folder.documents();
    let bytes = TemplateBundle::encode(VERSION, &folder.global, &folder.custom, &templates);
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    std::fs::write(&output, &bytes).unwrap();
    println!(
        "wrote {} templates, {} bytes, to {}",
        templates.len(),
        bytes.len(),
        output.display()
    );
}
