use std::path::{Path, PathBuf};

use missal_core::{Document, FieldCatalog, FieldMarkers};

pub struct WordImport {
    pub document: Document,
    pub notices: Vec<&'static str>,
    pub name: Option<String>,
    pub file_name: String,
    pub field_count: usize,
    pub unrecognised: Vec<String>,
}

const LONGEST_MARKER_NAME: usize = 40;

pub fn unrecognised_markers(text: &str, markers: &FieldMarkers) -> Vec<String> {
    let (open, close) = (markers.open(), markers.close());
    let mut found: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(open) {
        let after = &rest[start + open.len()..];
        let Some(end) = after.find(close) else {
            break;
        };
        let name = &after[..end];
        let plausible = !name.trim().is_empty()
            && !name.contains(open)
            && !name.contains('\n')
            && name.chars().count() <= LONGEST_MARKER_NAME;
        if plausible {
            let token = markers.token(name);
            if !found.contains(&token) {
                found.push(token);
            }
            rest = &after[end + close.len()..];
        } else {
            rest = after;
        }
    }
    found
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
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let projections = imported.document.projections();
    let unrecognised = unrecognised_markers(&projections.plain_text, markers);
    Ok(WordImport {
        field_count: projections.field_count,
        unrecognised,
        file_name,
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
    fn leftover_marker_tokens_are_reported_once_each() {
        let markers = FieldMarkers::default();
        let found = unrecognised_markers("a «جرم» b «جرم» c «نام ملزم» d « e », f »", &markers);
        assert_eq!(found, ["«جرم»", "«نام ملزم»", "« e »"]);
        assert!(unrecognised_markers("plain text", &markers).is_empty());
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
