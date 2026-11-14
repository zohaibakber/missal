use gpui_kit::{Context, EventEmitter};
use missal_core::TemplateId;
use missal_store::TemplateSummary;

use super::{Load, run_db};

pub enum TemplatesEvent {
    Changed,
}

pub struct Templates {
    pub items: Vec<TemplateSummary>,
    pub load: Load,
}

impl EventEmitter<TemplatesEvent> for Templates {}

impl Templates {
    pub fn new(cx: &mut Context<Self>) -> Templates {
        let mut templates = Templates {
            items: Vec::new(),
            load: Load::Loading,
        };
        templates.reload(cx);
        templates
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        run_db(
            cx,
            |store| store.templates(),
            |this, result, cx| {
                match result {
                    Ok(items) => {
                        this.items = items;
                        this.load = Load::Ready;
                    }
                    Err(error) => this.load = Load::Failed(error.message()),
                }
                this.changed(cx);
            },
        );
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(TemplatesEvent::Changed);
        cx.notify();
    }

    pub fn upsert(&mut self, summary: TemplateSummary, cx: &mut Context<Self>) {
        self.items.retain(|item| item.id != summary.id);
        self.items.insert(0, summary);
        self.load = Load::Ready;
        self.changed(cx);
    }

    pub fn remove(&mut self, id: TemplateId, cx: &mut Context<Self>) {
        self.items.retain(|item| item.id != id);
        self.changed(cx);
    }

    pub fn get(&self, id: TemplateId) -> Option<&TemplateSummary> {
        self.items.iter().find(|item| item.id == id)
    }
}
