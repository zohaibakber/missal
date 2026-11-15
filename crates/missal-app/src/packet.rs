use std::ops::Range;
use std::sync::Arc;

use gpui_kit::{App, Task};
use missal_core::{
    DisplayMode, Document, FieldCatalog, FieldPresentation, FieldReference, FirDocumentId,
    FirValues,
};
use missal_page::{DocSource, Packet, Page, PageEngine, shaping_cache};

use crate::state::db;

pub struct PacketJob {
    pub catalog: Arc<FieldCatalog>,
    pub values: Option<Arc<FirValues>>,
    pub mode: DisplayMode,
    pub documents: Vec<Arc<Document>>,
}

pub struct LaidOut {
    pub pages: Vec<Page>,
    pub ranges: Vec<Range<usize>>,
    pub empty: Vec<Vec<String>>,
}

impl PacketJob {
    fn presentation(&self) -> FieldPresentation<'_> {
        FieldPresentation::new(&self.catalog, self.values.as_deref(), self.mode)
    }

    fn layout_with(&self, engine: &mut PageEngine) -> Packet {
        let presentation = self.presentation();
        let sources: Vec<DocSource<'_>> = self
            .documents
            .iter()
            .map(|document| DocSource::new(document.as_ref(), &presentation))
            .collect();
        engine.layout(&sources)
    }

    pub fn lay_out(&self) -> LaidOut {
        let packet = self.layout_with(&mut PageEngine::new());
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
        if self.values.is_none() || self.mode != DisplayMode::Values {
            return vec![Vec::new(); self.documents.len()];
        }
        let presentation = self.presentation();
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

pub fn prewarm(
    documents: Vec<FirDocumentId>,
    catalog: Arc<FieldCatalog>,
    values: Arc<FirValues>,
    mode: DisplayMode,
    cx: &App,
) -> Task<()> {
    if documents.is_empty() {
        return Task::ready(());
    }
    let reply = db(cx).run(move |store| store.fir_documents_by_id(&documents));
    let executor = cx.background_executor().clone();
    cx.background_executor().spawn(async move {
        let Ok(records) = reply.await else {
            return;
        };
        let documents: Vec<Arc<Document>> = records
            .into_iter()
            .map(|record| Arc::new(record.document))
            .collect();
        let mut engine = PageEngine::with_cache(shaping_cache().speculative());
        for mode in [mode, mode.toggled()] {
            for document in &documents {
                let job = PacketJob {
                    catalog: catalog.clone(),
                    values: Some(values.clone()),
                    mode,
                    documents: vec![document.clone()],
                };
                engine = executor
                    .spawn(async move {
                        job.layout_with(&mut engine);
                        engine
                    })
                    .await;
            }
        }
    })
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
