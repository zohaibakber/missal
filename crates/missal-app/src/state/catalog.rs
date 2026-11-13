use std::rc::Rc;

use gpui_kit::{Context, EventEmitter};
use missal_core::{
    AppSettings, FieldCatalog, GlobalPlaceholder, Placeholder, PlaceholderSource,
};

use super::{Load, run_db};

pub enum CatalogEvent {
    Changed,
}

pub struct Catalog {
    pub settings: AppSettings,
    pub placeholders: Vec<Placeholder>,
    pub globals: Vec<GlobalPlaceholder>,
    pub load: Load,
    field_catalog: Rc<FieldCatalog>,
}

impl EventEmitter<CatalogEvent> for Catalog {}

impl Catalog {
    pub fn new(cx: &mut Context<Self>) -> Catalog {
        let mut catalog = Catalog {
            settings: AppSettings::default(),
            placeholders: Vec::new(),
            globals: Vec::new(),
            load: Load::Loading,
            field_catalog: Rc::new(FieldCatalog::default()),
        };
        catalog.reload(cx);
        catalog
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        run_db(
            cx,
            |store| Ok((store.settings()?, store.placeholders()?, store.global_placeholders()?)),
            |this, result, cx| {
                match result {
                    Ok((settings, placeholders, globals)) => {
                        this.apply(settings, placeholders, globals);
                    }
                    Err(error) => this.load = Load::Failed(error.message()),
                }
                this.changed(cx);
            },
        );
    }

    pub fn apply(
        &mut self,
        settings: AppSettings,
        placeholders: Vec<Placeholder>,
        globals: Vec<GlobalPlaceholder>,
    ) {
        self.field_catalog = Rc::new(FieldCatalog::new(placeholders.clone()));
        self.settings = settings;
        self.placeholders = placeholders;
        self.globals = globals;
        self.load = Load::Ready;
    }

    pub fn set_settings(&mut self, settings: AppSettings, cx: &mut Context<Self>) {
        self.settings = settings;
        self.changed(cx);
    }

    pub fn replace_all(
        &mut self,
        settings: AppSettings,
        placeholders: Vec<Placeholder>,
        globals: Vec<GlobalPlaceholder>,
        cx: &mut Context<Self>,
    ) {
        self.apply(settings, placeholders, globals);
        self.changed(cx);
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(CatalogEvent::Changed);
        cx.notify();
    }

    pub fn field_catalog(&self) -> Rc<FieldCatalog> {
        self.field_catalog.clone()
    }

    pub fn custom_fields(&self) -> impl Iterator<Item = &Placeholder> {
        self.placeholders
            .iter()
            .filter(|placeholder| placeholder.source == PlaceholderSource::Custom)
    }
}
