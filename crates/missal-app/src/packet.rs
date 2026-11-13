use missal_core::{DisplayMode, Document, FieldCatalog, FieldPresentation, FirValues};
use missal_page::{DocSource, Packet, Page, PageEngine};

pub struct PacketJob {
    pub catalog: FieldCatalog,
    pub values: Option<FirValues>,
    pub mode: DisplayMode,
    pub documents: Vec<Document>,
}

pub struct LaidOut {
    pub packet: Packet,
    pub pages: Vec<Page>,
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
        LaidOut { packet, pages }
    }
}

fn _assert_send() {
    fn send<T: Send>() {}
    send::<PacketJob>();
    send::<LaidOut>();
    fn sync<T: Sync + Send>() {}
    sync::<Packet>();
}
