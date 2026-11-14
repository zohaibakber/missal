use std::path::{Path, PathBuf};

use missal_core::{Document, FieldCatalog, FieldMarkers};

pub struct WordImport {
    pub document: Document,
    pub notices: Vec<&'static str>,
    pub name: Option<String>,
}

pub fn is_docx(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("docx"))
}

pub fn file_stem(path: &Path) -> Option<String> {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().trim().to_owned())
        .filter(|stem| !stem.is_empty())
}

pub fn import_file(
    path: PathBuf,
    catalog: &FieldCatalog,
    markers: &FieldMarkers,
) -> Result<WordImport, String> {
    if !is_docx(&path) {
        return Err("Choose a Word document with the .docx extension.".to_owned());
    }
    let bytes =
        std::fs::read(&path).map_err(|error| format!("The file could not be read ({error})."))?;
    let imported =
        missal_docx::import(&bytes, catalog, markers).map_err(|error| error.to_string())?;
    let name = imported
        .suggested_name
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .or_else(|| file_stem(&path));
    Ok(WordImport {
        document: imported.document,
        notices: imported
            .notices
            .into_iter()
            .map(|notice| notice.message())
            .collect(),
        name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_docx_files_are_accepted_regardless_of_case() {
        assert!(is_docx(Path::new("a/b.docx")));
        assert!(is_docx(Path::new("B.DOCX")));
        assert!(!is_docx(Path::new("b.doc")));
        assert!(!is_docx(Path::new("docx")));
    }

    #[test]
    fn the_file_name_without_extension_names_the_template() {
        assert_eq!(
            file_stem(Path::new("/x/ضمانت درخواست.docx")).as_deref(),
            Some("ضمانت درخواست")
        );
    }

    #[test]
    fn a_non_docx_choice_is_rejected_before_reading() {
        let result = import_file(
            PathBuf::from("/nonexistent/file.txt"),
            &FieldCatalog::default(),
            &FieldMarkers::default(),
        );
        assert!(result.is_err_and(|message| message.contains(".docx")));
    }
}
