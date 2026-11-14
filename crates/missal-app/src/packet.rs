use std::ops::Range;

use missal_core::{
    DisplayMode, Document, FieldCatalog, FieldPresentation, FieldReference, FirValues,
};
use missal_page::{DocSource, Page, PageEngine};

pub struct PacketJob {
    pub catalog: FieldCatalog,
    pub values: Option<FirValues>,
    pub mode: DisplayMode,
    pub documents: Vec<Document>,
}

pub struct LaidOut {
    pub pages: Vec<Page>,
    pub ranges: Vec<Range<usize>>,
    pub empty: Vec<Vec<String>>,
}

impl PacketJob {
    pub fn lay_out(&self) -> LaidOut {
        let presentation = match &self.values {
            Some(values) => FieldPresentation::for_fir(&self.catalog, values, self.mode),
            None => FieldPresentation::labels(&self.catalog),
        };
        let sources: Vec<DocSource<'_>> = self
            .documents
            .iter()
            .map(|document| DocSource::new(document, &presentation))
            .collect();
        let packet = PageEngine::new().layout(&sources);
        let pages = packet.pages().cloned().collect();
        let ranges = (0..self.documents.len())
            .map(|document| packet.document_pages(document))
            .collect();
        LaidOut {
            pages,
            ranges,
            empty: self.empty_fields(),
        }
    }

    pub fn empty_fields(&self) -> Vec<Vec<String>> {
        let Some(values) = self
            .values
            .as_ref()
            .filter(|_| self.mode == DisplayMode::Values)
        else {
            return vec![Vec::new(); self.documents.len()];
        };
        let presentation = FieldPresentation::for_fir(&self.catalog, values, self.mode);
        self.documents
            .iter()
            .map(|document| {
                let mut names: Vec<String> = Vec::new();
                for reference in document.field_references() {
                    let fillable = match &reference {
                        FieldReference::Fir(_) => true,
                        FieldReference::Catalog(id) => self.catalog.placeholder(*id).is_some(),
                        FieldReference::Unresolved(_) => false,
                    };
                    if !fillable {
                        continue;
                    }
                    let shown = presentation.display_text(&reference);
                    if shown.unresolved && !names.contains(&shown.text) {
                        names.push(shown.text);
                    }
                }
                names
            })
            .collect()
    }
}

pub fn distinct_empty<'a>(lists: impl IntoIterator<Item = &'a Vec<String>>) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for list in lists {
        for name in list {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
    }
    names
}
