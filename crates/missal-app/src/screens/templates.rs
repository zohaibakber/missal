use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::component::{Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::TestSupportExt as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::menu::DropdownMenu as _;

use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, ScrollStrategy, StatefulInteractiveElement, Styled,
    Subscription, UniformListScrollHandle, Window, div, px, uniform_list,
};
use missal_core::{Document, Revision, TemplateId};
use missal_store::TemplateSummary;
use missal_text::{TextInput, TextInputEvent, label};

use crate::app::{navigate, navigate_replace};
use crate::editor::{DocumentEditor, EditorEvent, EditorFactory, FieldContext, LoadRequest};
use crate::shortcuts::*;
use crate::state::rows::fold_for_search;
use crate::state::templates::TemplatesEvent;
use crate::state::{Load, Models, Route, not_available_yet, run_db_in, toast_error, toast_success};
use crate::theme::{Palette, RAIL_WIDTH};
use crate::ui::dialogs::confirm_delete;
use crate::ui::input::{Kind, text_input};
use crate::ui::{empty_state, icon_button, pane_header, rail_header};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PageState {
    Loading,
    Ready,
    NotFound,
    Failed(String),
}

pub struct TemplatePage {
    models: Models,
    id: Option<TemplateId>,
    title: Entity<TextInput>,
    initial_name: String,
    revision: Option<Revision>,
    state: PageState,
    editor: Rc<dyn DocumentEditor>,
    editor_dirty: bool,
    saving: bool,
    _subscriptions: Vec<Subscription>,
}

impl TemplatePage {
    pub fn new(models: Models, id: Option<TemplateId>, window: &mut Window, cx: &mut Context<Self>) -> TemplatePage {
        let title = text_input(window, cx, Kind::Urdu, "ٹیمپلیٹ کا نام", "");
        let editor = EditorFactory::create(cx).build(window, cx);
        let weak = cx.entity().downgrade();
        let editor_subscription = editor.on_event(cx, {
            let weak = weak.clone();
            Box::new(move |event, cx| {
                let event = event.clone();
                weak.update(cx, |this, cx| {
                    if let EditorEvent::DirtyChanged(dirty) = event {
                        this.editor_dirty = dirty;
                        cx.notify();
                    }
                })
                .ok();
            })
        });
        let title_subscription = cx.subscribe(&title, |_, _, event: &TextInputEvent, cx| {
            if *event == TextInputEvent::Change {
                cx.notify();
            }
        });
        let mut page = TemplatePage {
            models,
            id,
            title,
            initial_name: String::new(),
            revision: None,
            state: PageState::Loading,
            editor,
            editor_dirty: false,
            saving: false,
            _subscriptions: vec![editor_subscription, title_subscription],
        };
        page.load(window, cx);
        page
    }

    fn field_context(&self, cx: &App) -> FieldContext {
        let catalog = self.models.catalog.read(cx);
        FieldContext::labels(catalog.field_catalog(), catalog.settings.field_markers.clone())
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.id {
            None => {
                self.state = PageState::Ready;
                let request = LoadRequest {
                    key: "template:new".into(),
                    document: Document::empty(),
                    fields: self.field_context(cx),
                };
                self.editor.load(request, window, cx);
                self.title.update(cx, |input, cx| input.focus(window, cx));
            }
            Some(id) => {
                run_db_in(window, cx, move |store| store.template(id), move |this, result, window, cx| {
                    match result {
                        Ok(record) => {
                            this.revision = Some(record.revision);
                            this.initial_name = record.name.clone();
                            this.title.update(cx, |input, cx| input.set_text(record.name.clone(), cx));
                            let request = LoadRequest {
                                key: format!("template:{}:{}", record.id, record.revision),
                                document: record.document,
                                fields: this.field_context(cx),
                            };
                            this.editor.load(request, window, cx);
                            this.state = PageState::Ready;
                        }
                        Err(missal_core::RepositoryError::NotFound { .. }) => this.state = PageState::NotFound,
                        Err(error) => this.state = PageState::Failed(error.message()),
                    }
                    cx.notify();
                });
            }
        }
    }

    pub fn is_dirty(&self, cx: &App) -> bool {
        self.state == PageState::Ready
            && (self.title.read(cx).text() != self.initial_name || self.editor_dirty || self.editor.is_dirty(cx))
    }

    pub fn title_input(&self) -> &Entity<TextInput> {
        &self.title
    }

    pub fn editor(&self) -> &Rc<dyn DocumentEditor> {
        &self.editor
    }

    pub fn state(&self) -> &PageState {
        &self.state
    }

    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.title.read(cx).text().trim().to_owned();
        if self.saving || name.is_empty() || self.state != PageState::Ready {
            return;
        }
        if self.id.is_some() && !self.is_dirty(cx) {
            return;
        }
        let snapshot = self.editor.snapshot(cx);
        self.saving = true;
        cx.notify();
        let revision_token = snapshot.revision;
        let document = snapshot.document;
        let id = self.id;
        let revision = self.revision;
        let job_name = name.clone();
        run_db_in(
            window,
            cx,
            move |store| match (id, revision) {
                (Some(id), Some(revision)) => {
                    let ack = store.save_template(id, revision, &job_name, &document)?;
                    Ok((None, Some(ack)))
                }
                _ => Ok((Some(store.create_template(&job_name, &document)?), None)),
            },
            move |this, result, window, cx| {
                this.saving = false;
                match result {
                    Ok((Some(summary), _)) => {
                        let new_id = summary.id;
                        this.models.templates.update(cx, |templates, cx| templates.upsert(summary, cx));
                        this.editor.mark_saved(revision_token, cx);
                        this.initial_name = name.clone();
                        toast_success(window, cx, "Template created");
                        navigate_replace(Route::Template(new_id), cx);
                    }
                    Ok((None, Some(ack))) => {
                        this.revision = Some(ack.revision);
                        this.initial_name = name.clone();
                        this.editor.mark_saved(revision_token, cx);
                        let created_at = this
                            .id
                            .and_then(|id| this.models.templates.read(cx).get(id).map(|item| item.created_at.clone()));
                        if let (Some(id), Some(created_at)) = (this.id, created_at) {
                            let summary = TemplateSummary {
                                id,
                                name: name.clone(),
                                revision: ack.revision,
                                preview_text: ack.preview_text,
                                field_count: ack.field_count,
                                created_at,
                                updated_at: ack.updated_at,
                            };
                            this.models.templates.update(cx, |templates, cx| templates.upsert(summary, cx));
                        }
                        toast_success(window, cx, "Template updated");
                    }
                    Ok(_) => {}
                    Err(error) => toast_error(window, cx, error.message()),
                }
                cx.notify();
            },
        );
    }

    fn request_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.id else {
            return;
        };
        let weak = cx.entity().downgrade();
        confirm_delete(
            window,
            cx,
            "Delete this template?",
            "This can't be undone. Templates that an FIR's documents use can't be deleted.",
            "Delete template",
            move |window, cx| {
                weak.update(cx, |this, cx| this.delete(id, window, cx)).ok();
            },
        );
    }

    fn delete(&mut self, id: TemplateId, window: &mut Window, cx: &mut Context<Self>) {
        run_db_in(window, cx, move |store| store.remove_template(id), move |this, result, window, cx| match result {
            Ok(()) => {
                this.models.templates.update(cx, |templates, cx| templates.remove(id, cx));
                this.initial_name = this.title.read(cx).text().to_owned();
                navigate_replace(Route::Templates, cx);
                toast_success(window, cx, "Template deleted");
            }
            Err(error) => toast_error(window, cx, error.message()),
        });
    }
}

impl Render for TemplatePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        match &self.state {
            PageState::NotFound => {
                return empty_state(&palette, IconName::FileText, "Template not found", None, None).into_any_element();
            }
            PageState::Failed(message) => {
                return empty_state(&palette, IconName::TriangleAlert, "Could not load this template", Some(message.clone().into()), None)
                    .into_any_element();
            }
            _ => {}
        }
        let dirty = self.is_dirty(cx);
        let name_empty = self.title.read(cx).text().trim().is_empty();
        let is_new = self.id.is_none();
        let save_enabled = !name_empty && (is_new || dirty) && !self.saving;
        let weak = cx.entity().downgrade();
        v_flex()
            .size_full()
            .child(
                pane_header(&palette)
                    .child(div().flex_1().min_w_0().child(self.title.clone()))
                    .child(div().w(px(8.)))
                    .children((!is_new).then(|| {
                        Button::new("template-more")
                            .ghost()
                            .small()
                            .icon(IconName::Ellipsis)
                            .tooltip("More actions")
                            .dropdown_menu({
                                let weak = weak.clone();
                                move |menu, _, _| {
                                    let weak = weak.clone();
                                    menu.item(PopupMenuItem::new("Delete template").icon(IconName::Trash).on_click(move |_, window, cx| {
                                        weak.update(cx, |this, cx| this.request_delete(window, cx)).ok();
                                    }))
                                }
                            })
                    }))
                    .child(
                        Button::new("template-save")
                            .primary()
                            .small()
                            .loading(self.saving)
                            .disabled(!save_enabled)
                            .label(if is_new { "Create template" } else if dirty { "Save" } else { "Saved" })
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    )
                    .child(icon_button("template-print", IconName::Printer).tooltip("Print preview").on_click(|_, window, cx| {
                        not_available_yet(window, cx, "Print preview");
                    }))
                    .child(
                        Button::new("template-import")
                            .outline()
                            .small()
                            .icon(IconName::FileUp)
                            .label("Upload Word file")
                            .on_click(|_, window, cx| not_available_yet(window, cx, "Word import")),
                    ),
            )
            .child(
                div()
                    .id("template-editor")
                    .flex_1()
                    .min_h_0()
                    .child(self.editor.view()),
            )
            .into_any_element()
    }
}

pub struct TemplatesScreen {
    models: Models,
    focus: FocusHandle,
    search: Entity<TextInput>,
    query: String,
    route: Route,
    page: Option<Entity<TemplatePage>>,
    scroll: UniformListScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl TemplatesScreen {
    pub fn new(models: Models, route: Route, window: &mut Window, cx: &mut Context<Self>) -> TemplatesScreen {
        let search = text_input(window, cx, Kind::Auto, "Search templates", "");
        let subscriptions = vec![
            cx.subscribe(&search, |this, input, event: &TextInputEvent, cx| {
                if *event == TextInputEvent::Change {
                    this.query = input.read(cx).text().to_owned();
                    cx.notify();
                }
            }),
            cx.subscribe(&models.templates, |_, _, _: &TemplatesEvent, cx| cx.notify()),
        ];
        let mut screen = TemplatesScreen {
            models,
            focus: cx.focus_handle(),
            search,
            query: String::new(),
            route,
            page: None,
            scroll: UniformListScrollHandle::new(),
            _subscriptions: subscriptions,
        };
        screen.build_page(window, cx);
        screen
    }

    fn build_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.page = match self.route {
            Route::NewTemplate => Some(cx.new(|cx| TemplatePage::new(self.models.clone(), None, window, cx))),
            Route::Template(id) => Some(cx.new(|cx| TemplatePage::new(self.models.clone(), Some(id), window, cx))),
            _ => None,
        };
        if let Some(page) = &self.page {
            cx.observe(page, |_, _, cx| cx.notify()).detach();
        }
        if let Route::Template(id) = self.route {
            let visible = self.visible(cx);
            if let Some(position) = visible.iter().position(|item| item.id == id) {
                self.scroll.scroll_to_item(position, ScrollStrategy::Center);
            }
        }
    }

    pub fn route_changed(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        self.route = route;
        self.build_page(window, cx);
        self.focus(window, cx);
        cx.notify();
    }

    pub fn is_dirty(&self, cx: &App) -> bool {
        self.page.as_ref().is_some_and(|page| page.read(cx).is_dirty(cx))
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        match (&self.page, self.route) {
            (Some(page), Route::NewTemplate) => {
                let title = page.read(cx).title.clone();
                title.update(cx, |input, cx| input.focus(window, cx));
            }
            _ => window.focus(&self.focus, cx),
        }
    }

    pub fn page(&self) -> Option<&Entity<TemplatePage>> {
        self.page.as_ref()
    }

    pub fn search_input(&self) -> &Entity<TextInput> {
        &self.search
    }

    pub fn visible(&self, cx: &App) -> Vec<TemplateSummary> {
        let words: Vec<String> = fold_for_search(&self.query).split_whitespace().map(str::to_owned).collect();
        self.models
            .templates
            .read(cx)
            .items
            .iter()
            .filter(|item| {
                words.is_empty() || {
                    let haystack = fold_for_search(&format!("{} {}", item.name, item.preview_text));
                    words.iter().all(|word| haystack.contains(word.as_str()))
                }
            })
            .cloned()
            .collect()
    }

    fn step(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self.visible(cx);
        if visible.is_empty() {
            return;
        }
        let current = match self.route {
            Route::Template(id) => visible.iter().position(|item| item.id == id),
            _ => None,
        };
        let next = match current {
            None => {
                if delta >= 0 { 0 } else { visible.len() - 1 }
            }
            Some(position) => (position as isize + delta).clamp(0, visible.len() as isize - 1) as usize,
        };
        if Some(next) != current {
            self.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
            navigate(Route::Template(visible[next].id), window, cx);
        }
    }

    fn rail(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = *Palette::get(cx);
        let load = self.models.templates.read(cx).load.clone();
        let visible = self.visible(cx);
        let total = self.models.templates.read(cx).items.len();
        let active = match self.route {
            Route::Template(id) => Some(id),
            _ => None,
        };
        let _ = window;
        let body: AnyElement = match load {
            Load::Loading => v_flex()
                .gap_2()
                .p_3()
                .children((0..5usize).map(|index| div().id(("template-skeleton", index)).h(px(48.)).rounded(px(7.)).bg(palette.muted)))
                .into_any_element(),
            Load::Failed(message) => empty_state(&palette, IconName::TriangleAlert, "Could not load templates", Some(message.into()), None).into_any_element(),
            Load::Ready if total == 0 => empty_state(&palette, IconName::FileText, "No templates yet", None, None).into_any_element(),
            Load::Ready if visible.is_empty() => empty_state(&palette, IconName::Search, "No matching templates", None, None).into_any_element(),
            Load::Ready => {
                let items = Rc::new(visible);
                uniform_list("template-rail", items.len(), {
                    let items = items.clone();
                    move |range, _, _| {
                        range
                            .filter_map(|index| {
                                let item = items.get(index)?.clone();
                                let is_active = active == Some(item.id);
                                let id = item.id;
                                Some(
                                    v_flex()
                                        .id(("template-item", index))
                                        .test_support()
                                        .h(px(56.))
                                        .mx_2()
                                        .px_2()
                                        .justify_center()
                                        .rounded(px(7.))
                                        .cursor_pointer()
                                        .when(is_active, |this| this.bg(palette.accent))
                                        .hover(move |style| style.bg(palette.accent))
                                        .on_click(move |_, window, cx| navigate(Route::Template(id), window, cx))
                                        .child(
                                            div()
                                                .overflow_hidden()
                                                .text_size(px(15.))
                                                .font_medium()
                                                .child(label(item.name.clone()).rtl().truncate()),
                                        )
                                        .child(
                                            div()
                                                .overflow_hidden()
                                                .text_size(px(12.))
                                                .text_color(palette.muted_foreground)
                                                .child(
                                                    label(if item.preview_text.is_empty() { "خالی".to_owned() } else { item.preview_text.clone() })
                                                        .rtl()
                                                        .truncate(),
                                                ),
                                        ),
                                )
                            })
                            .collect::<Vec<_>>()
                    }
                })
                .track_scroll(&self.scroll)
                .flex_1()
                .min_h_0()
                .into_any_element()
            }
        };
        v_flex()
            .id("template-rail-panel")
            .w(px(RAIL_WIDTH))
            .flex_shrink_0()
            .h_full()
            .border_l_1()
            .border_color(palette.border)
            .child(
                rail_header(&palette, "Templates")
                    .child(icon_button("new-template", IconName::Plus).tooltip("New template").on_click(|_, window, cx| {
                        navigate(Route::NewTemplate, window, cx);
                    })),
            )
            .child(
                div()
                    .id("template-search")
                    .px_3()
                    .pb_2()
                    .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        let key = event.keystroke.key.as_str();
                        let plain = !event.keystroke.modifiers.modified();
                        if key == "escape" && plain {
                            if this.query.is_empty() {
                                this.focus(window, cx);
                            } else {
                                this.search.update(cx, |input, cx| input.set_text("", cx));
                            }
                            cx.stop_propagation();
                        } else if key == "enter" && plain {
                            if let Some(first) = this.visible(cx).first() {
                                navigate(Route::Template(first.id), window, cx);
                            }
                            cx.stop_propagation();
                        } else if key == "down" && plain {
                            this.focus(window, cx);
                            this.step(1, window, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child(self.search.clone()),
            )
            .child(body)
            .into_any_element()
    }
}

impl Render for TemplatesScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let main: AnyElement = match &self.page {
            Some(page) => page.clone().into_any_element(),
            None => empty_state(
                &palette,
                IconName::FileText,
                "Select a template",
                Some("Choose a template from the list, or start a new one.".into()),
                Some(
                    Button::new("empty-new-template")
                        .outline()
                        .icon(IconName::Plus)
                        .label("New template")
                        .on_click(|_, window, cx| navigate(Route::NewTemplate, window, cx))
                        .into_any_element(),
                ),
            )
            .into_any_element(),
        };
        let rail = self.rail(window, cx);
        h_flex()
            .id("templates")
            .key_context(TEMPLATES)
            .track_focus(&self.focus)
            .size_full()
            .on_action(cx.listener(|this, _: &NextTemplate, window, cx| this.step(1, window, cx)))
            .on_action(cx.listener(|this, _: &PreviousTemplate, window, cx| this.step(-1, window, cx)))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.search.update(cx, |input, cx| {
                    input.focus(window, cx);
                    let length = input.text().len();
                    input.select(0..length, cx);
                });
            }))
            .on_action(cx.listener(|this, _: &Save, window, cx| {
                if let Some(page) = &this.page {
                    page.update(cx, |page, cx| page.save(window, cx));
                }
            }))
            .child(div().flex_1().min_w_0().h_full().child(main))
            .child(rail)
    }
}
