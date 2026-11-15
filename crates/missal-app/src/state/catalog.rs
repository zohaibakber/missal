use std::sync::Arc;

use gpui_kit::{Context, EventEmitter};
use missal_core::{AppSettings, FieldCatalog, GlobalPlaceholder, Placeholder, PlaceholderSource};

use super::{Load, run_db};

pub enum CatalogEvent {
    Changed,
}

pub struct Catalog {
    pub settings: AppSettings,
    pub globals: Vec<GlobalPlaceholder>,
    pub load: Load,
    field_catalog: Arc<FieldCatalog>,
}

impl EventEmitter<CatalogEvent> for Catalog {}

impl Catalog {
    pub fn empty() -> Catalog {
        Catalog {
            settings: AppSettings::default(),
            globals: Vec::new(),
            load: Load::Loading,
            field_catalog: Arc::new(FieldCatalog::default()),
        }
    }

    pub fn loaded(
        &mut self,
        result: Result<(AppSettings, Vec<Placeholder>, Vec<GlobalPlaceholder>), String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok((settings, placeholders, globals)) => self.apply(settings, placeholders, globals),
            Err(message) => self.load = Load::Failed(message),
        }
        self.changed(cx);
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        run_db(
            cx,
            |store| {
                Ok((
                    store.settings()?,
                    store.placeholders()?,
                    store.global_placeholders()?,
                ))
            },
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
        self.field_catalog = Arc::new(FieldCatalog::new(placeholders));
        self.settings = settings;
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

    pub fn field_catalog(&self) -> Arc<FieldCatalog> {
        self.field_catalog.clone()
    }

    pub fn placeholders(&self) -> &[Placeholder] {
        self.field_catalog.placeholders()
    }

    pub fn custom_fields(&self) -> impl Iterator<Item = &Placeholder> {
        self.field_catalog
            .placeholders()
            .iter()
            .filter(|placeholder| placeholder.source == PlaceholderSource::Custom)
    }
}
