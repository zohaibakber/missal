use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Context, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    Render, StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};
use missal_core::{
    CustomFieldDraft, FIR_FIELDS, FieldMarkers, GlobalPlaceholder, GlobalPlaceholderDraft,
    Placeholder, PlaceholderId,
};
use missal_text::{TextInput, TextInputEvent, label};

use crate::app::set_theme;
use crate::prefs::ThemeChoice;
use crate::shortcuts::*;
use crate::state::{Load, Models, run_db_in, toast_error, toast_success, toast_warning};
use crate::theme::{Palette, oklch};
use crate::ui::input::{Kind, text_input};
use crate::ui::{pane_header, save_status};

struct GlobalRow {
    id: Option<PlaceholderId>,
    name: Entity<TextInput>,
    value: Entity<TextInput>,
    initial: (String, String),
}

struct CustomRow {
    id: Option<PlaceholderId>,
    name: Entity<TextInput>,
    initial: String,
}

pub struct SettingsScreen {
    models: Models,
    focus: FocusHandle,
    marker_open: Entity<TextInput>,
    marker_close: Entity<TextInput>,
    marker_error: bool,
    marker_initial: (String, String),
    globals: Vec<GlobalRow>,
    customs: Vec<CustomRow>,
    synced: bool,
    saving: [bool; 3],
    subscriptions: Vec<Subscription>,
    row_subscriptions: Vec<Subscription>,
}

impl SettingsScreen {
    pub fn new(models: Models, window: &mut Window, cx: &mut Context<Self>) -> SettingsScreen {
        let marker_open = text_input(window, cx, Kind::Latin, "", "«");
        let marker_close = text_input(window, cx, Kind::Latin, "", "»");
        let subscriptions = vec![
            cx.observe(&models.catalog, |this, _, cx| {
                if !this.is_dirty(cx) {
                    this.synced = false;
                }
                cx.notify();
            }),
            cx.subscribe(&marker_open, |this, input, event: &TextInputEvent, cx| {
                this.marker_changed(&input, event, cx)
            }),
            cx.subscribe(&marker_close, |this, input, event: &TextInputEvent, cx| {
                this.marker_changed(&input, event, cx)
            }),
        ];
        SettingsScreen {
            models,
            focus: cx.focus_handle(),
            marker_open,
            marker_close,
            marker_error: false,
            marker_initial: (String::new(), String::new()),
            globals: Vec::new(),
            customs: Vec::new(),
            synced: false,
            saving: [false; 3],
            subscriptions,
            row_subscriptions: Vec::new(),
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    fn marker_changed(
        &mut self,
        input: &Entity<TextInput>,
        event: &TextInputEvent,
        cx: &mut Context<Self>,
    ) {
        if *event != TextInputEvent::Change {
            return;
        }
        let text = input.read(cx).text().to_owned();
        let trimmed = text.trim().to_owned();
        if trimmed != text {
            input.update(cx, |input, cx| input.set_text(trimmed, cx));
        }
        self.marker_error = false;
        cx.notify();
    }

    fn watch(&mut self, input: &Entity<TextInput>, cx: &mut Context<Self>) {
        let subscription = cx.subscribe(input, |_, _, event: &TextInputEvent, cx| {
            if *event == TextInputEvent::Change {
                cx.notify();
            }
        });
        self.row_subscriptions.push(subscription);
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let catalog = self.models.catalog.read(cx);
        if !catalog.load.is_ready() {
            return;
        }
        self.synced = true;
        let markers = catalog.settings.field_markers.clone();
        let globals: Vec<GlobalPlaceholder> = catalog.globals.clone();
        let customs: Vec<Placeholder> = catalog.custom_fields().cloned().collect();
        self.marker_open
            .update(cx, |input, cx| input.set_text(markers.open(), cx));
        self.marker_close
            .update(cx, |input, cx| input.set_text(markers.close(), cx));
        self.marker_initial = (markers.open().to_owned(), markers.close().to_owned());
        self.row_subscriptions.clear();
        self.globals = globals
            .into_iter()
            .map(|global| {
                self.global_row(Some(global.id), &global.label, &global.value, window, cx)
            })
            .collect();
        self.customs = customs
            .into_iter()
            .map(|field| self.custom_row(Some(field.id), &field.label, window, cx))
            .collect();
    }

    fn global_row(
        &mut self,
        id: Option<PlaceholderId>,
        name: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> GlobalRow {
        let name_input = text_input(window, cx, Kind::Urdu, "نام", name);
        let value_input = text_input(window, cx, Kind::Urdu, "قدر", value);
        self.watch(&name_input, cx);
        self.watch(&value_input, cx);
        GlobalRow {
            id,
            name: name_input,
            value: value_input,
            initial: (name.to_owned(), value.to_owned()),
        }
    }

    fn custom_row(
        &mut self,
        id: Option<PlaceholderId>,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> CustomRow {
        let input = text_input(window, cx, Kind::Urdu, "نام", name);
        self.watch(&input, cx);
        CustomRow {
            id,
            name: input,
            initial: name.to_owned(),
        }
    }

    pub fn markers_dirty(&self, cx: &App) -> bool {
        self.synced
            && (
                self.marker_open.read(cx).text().to_owned(),
                self.marker_close.read(cx).text().to_owned(),
            ) != self.marker_initial
    }

    pub fn globals_dirty(&self, cx: &App) -> bool {
        self.globals.iter().any(|row| {
            row.id.is_none()
                || (
                    row.name.read(cx).text().to_owned(),
                    row.value.read(cx).text().to_owned(),
                ) != row.initial
        })
    }

    pub fn customs_dirty(&self, cx: &App) -> bool {
        self.customs
            .iter()
            .any(|row| row.id.is_none() || row.name.read(cx).text() != row.initial)
    }

    pub fn is_dirty(&self, cx: &App) -> bool {
        self.markers_dirty(cx) || self.globals_dirty(cx) || self.customs_dirty(cx)
    }

    pub fn global_inputs(&self) -> Vec<(Entity<TextInput>, Entity<TextInput>)> {
        self.globals
            .iter()
            .map(|row| (row.name.clone(), row.value.clone()))
            .collect()
    }

    pub fn marker_inputs(&self) -> (Entity<TextInput>, Entity<TextInput>) {
        (self.marker_open.clone(), self.marker_close.clone())
    }

    pub fn custom_inputs(&self) -> Vec<Entity<TextInput>> {
        self.customs.iter().map(|row| row.name.clone()).collect()
    }

    pub fn add_global(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let row = self.global_row(None, "", "", window, cx);
        row.name.update(cx, |input, cx| input.focus(window, cx));
        self.globals.push(row);
        cx.notify();
    }

    pub fn add_custom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let row = self.custom_row(None, "", window, cx);
        row.name.update(cx, |input, cx| input.focus(window, cx));
        self.customs.push(row);
        cx.notify();
    }

    fn reset_globals(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.row_subscriptions.clear();
        let catalog = self.models.catalog.read(cx);
        let globals = catalog.globals.clone();
        let customs: Vec<Placeholder> = catalog.custom_fields().cloned().collect();
        let existing_customs_dirty = self.customs_dirty(cx);
        self.globals = globals
            .into_iter()
            .map(|global| {
                self.global_row(Some(global.id), &global.label, &global.value, window, cx)
            })
            .collect();
        let _ = (customs, existing_customs_dirty);
        self.rebuild_customs(window, cx);
        cx.notify();
    }

    fn rebuild_customs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let customs: Vec<Placeholder> = self
            .models
            .catalog
            .read(cx)
            .custom_fields()
            .cloned()
            .collect();
        self.customs = customs
            .into_iter()
            .map(|field| self.custom_row(Some(field.id), &field.label, window, cx))
            .collect();
    }

    pub fn save_markers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open = self.marker_open.read(cx).text().to_owned();
        let close = self.marker_close.read(cx).text().to_owned();
        let Ok(markers) = FieldMarkers::new(&open, &close) else {
            self.marker_error = true;
            cx.notify();
            return;
        };
        self.saving[0] = true;
        run_db_in(
            window,
            cx,
            move |store| store.save_field_markers(&markers),
            |this, result, window, cx| {
                this.saving[0] = false;
                match result {
                    Ok(settings) => {
                        this.marker_initial = (
                            settings.field_markers.open().to_owned(),
                            settings.field_markers.close().to_owned(),
                        );
                        this.models
                            .catalog
                            .update(cx, |catalog, cx| catalog.set_settings(settings, cx));
                        toast_success(window, cx, "Placeholder markers saved");
                    }
                    Err(error) => toast_error(window, cx, error.message()),
                }
                cx.notify();
            },
        );
    }

    pub fn save_globals(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut drafts = Vec::new();
        for row in &self.globals {
            let name = row.name.read(cx).text().trim().to_owned();
            let value = row.value.read(cx).text().to_owned();
            if name.is_empty() {
                toast_warning(window, cx, "Give every global placeholder a name");
                return;
            }
            match row.id {
                Some(id) if (name.clone(), value.clone()) != row.initial => {
                    drafts.push(GlobalPlaceholderDraft::Existing {
                        id,
                        label: name,
                        value,
                    });
                }
                None => drafts.push(GlobalPlaceholderDraft::New { label: name, value }),
                Some(_) => {}
            }
        }
        self.saving[1] = true;
        cx.notify();
        run_db_in(
            window,
            cx,
            move |store| {
                store.save_global_placeholders(&drafts)?;
                Ok((
                    store.settings()?,
                    store.placeholders()?,
                    store.global_placeholders()?,
                ))
            },
            |this, result, window, cx| {
                this.saving[1] = false;
                match result {
                    Ok((settings, placeholders, globals)) => {
                        this.models.catalog.update(cx, |catalog, cx| {
                            catalog.replace_all(settings, placeholders, globals, cx)
                        });
                        this.row_subscriptions.clear();
                        this.reset_globals(window, cx);
                        toast_success(window, cx, "Global values saved");
                    }
                    Err(error) => toast_error(window, cx, error.message()),
                }
                cx.notify();
            },
        );
    }

    pub fn save_customs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut drafts = Vec::new();
        for row in &self.customs {
            let name = row.name.read(cx).text().trim().to_owned();
            if name.is_empty() {
                toast_warning(window, cx, "Enter a name");
                return;
            }
            match row.id {
                Some(id) if name != row.initial => {
                    drafts.push(CustomFieldDraft::Rename { id, label: name })
                }
                None => drafts.push(CustomFieldDraft::New { label: name }),
                Some(_) => {}
            }
        }
        self.saving[2] = true;
        cx.notify();
        run_db_in(
            window,
            cx,
            move |store| {
                store.save_custom_fields(&drafts)?;
                Ok((
                    store.settings()?,
                    store.placeholders()?,
                    store.global_placeholders()?,
                ))
            },
            |this, result, window, cx| {
                this.saving[2] = false;
                match result {
                    Ok((settings, placeholders, globals)) => {
                        this.models.catalog.update(cx, |catalog, cx| {
                            catalog.replace_all(settings, placeholders, globals, cx)
                        });
                        this.row_subscriptions.clear();
                        this.reset_globals(window, cx);
                        toast_success(window, cx, "Placeholders saved");
                    }
                    Err(error) => toast_error(window, cx, error.message()),
                }
                cx.notify();
            },
        );
    }

    fn save_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.markers_dirty(cx) {
            self.save_markers(window, cx);
        }
        if self.globals_dirty(cx) {
            self.save_globals(window, cx);
        }
        if self.customs_dirty(cx) {
            self.save_customs(window, cx);
        }
    }

    fn section(
        &self,
        palette: &Palette,
        title: &'static str,
        description: &'static str,
        body: impl IntoElement,
    ) -> impl IntoElement {
        h_flex()
            .items_start()
            .gap_8()
            .py_6()
            .border_b_1()
            .border_color(palette.border)
            .child(
                v_flex()
                    .w(px(220.))
                    .flex_shrink_0()
                    .gap_1()
                    .child(div().text_size(px(14.)).font_semibold().child(title))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(palette.muted_foreground)
                            .child(description),
                    ),
            )
            .child(v_flex().flex_1().min_w_0().gap_3().child(body))
    }

    fn theme_card(
        &self,
        palette: &Palette,
        choice: ThemeChoice,
        current: ThemeChoice,
    ) -> impl IntoElement {
        let palette = *palette;
        let selected = choice == current;
        let ink = |l: f32| oklch(l, 0.0, 0.0, 1.0);
        let preview = |bg: gpui_kit::Hsla, bar: gpui_kit::Hsla, line: gpui_kit::Hsla| {
            v_flex()
                .size_full()
                .bg(bg)
                .child(div().h(px(10.)).bg(bar))
                .child(
                    v_flex()
                        .p(px(7.))
                        .gap(px(5.))
                        .child(div().h(px(5.)).w(px(46.)).rounded(px(2.)).bg(line))
                        .child(div().h(px(5.)).w(px(30.)).rounded(px(2.)).bg(line)),
                )
        };
        let light = || preview(ink(0.98), ink(0.922), ink(0.87));
        let dark = || preview(ink(0.205), ink(0.269), ink(0.439));
        let face: AnyElement = match choice {
            ThemeChoice::Light => light().into_any_element(),
            ThemeChoice::Dark => dark().into_any_element(),
            ThemeChoice::System => h_flex()
                .size_full()
                .child(
                    div()
                        .w(px(44.))
                        .h_full()
                        .overflow_hidden()
                        .child(div().w(px(88.)).h_full().child(light())),
                )
                .child(
                    div()
                        .w(px(44.))
                        .h_full()
                        .overflow_hidden()
                        .child(div().w(px(88.)).h_full().ml(px(-44.)).child(dark())),
                )
                .into_any_element(),
        };
        v_flex()
            .id(("theme", choice as usize))
            .test_support()
            .gap_1p5()
            .items_center()
            .cursor_pointer()
            .on_click(move |_, window, cx| set_theme(choice, window, cx))
            .child(
                div()
                    .w(px(88.))
                    .h(px(70.))
                    .rounded(px(8.))
                    .overflow_hidden()
                    .border_2()
                    .border_color(if selected {
                        palette.primary
                    } else {
                        palette.border
                    })
                    .child(face),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .when(selected, |this| this.font_medium())
                    .text_color(if selected {
                        palette.foreground
                    } else {
                        palette.muted_foreground
                    })
                    .child(choice.title()),
            )
    }

    #[allow(clippy::too_many_arguments)]
    fn section_actions(
        &self,
        palette: &Palette,
        dirty: bool,
        saving: bool,
        id: &'static str,
        save: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        reset: Option<fn(&mut Self, &mut Window, &mut Context<Self>)>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        (dirty || saving).then(|| {
            h_flex()
                .items_center()
                .gap_2()
                .child(save_status(palette, true))
                .child(div().flex_1())
                .children(reset.map(|reset| {
                    Button::new((id, 1usize))
                        .outline()
                        .small()
                        .label("Reset")
                        .on_click(cx.listener(move |this, _, window, cx| reset(this, window, cx)))
                }))
                .child(
                    Button::new((id, 2usize))
                        .primary()
                        .small()
                        .loading(saving)
                        .label("Save")
                        .on_click(cx.listener(move |this, _, window, cx| save(this, window, cx))),
                )
                .into_any_element()
        })
    }
}

impl Render for SettingsScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let load = self.models.catalog.read(cx).load.clone();
        if load.is_ready() && !self.synced {
            self.sync(window, cx);
        }
        let theme = self.models.prefs.read(cx).read().theme;
        let (markers_dirty, globals_dirty, customs_dirty) = (
            self.markers_dirty(cx),
            self.globals_dirty(cx),
            self.customs_dirty(cx),
        );
        let marker_text = (
            self.marker_open.read(cx).text().to_owned(),
            self.marker_close.read(cx).text().to_owned(),
        );
        let markers_preview = FieldMarkers::new(&marker_text.0, &marker_text.1).unwrap_or_default();
        let chips_markers = self.models.catalog.read(cx).settings.field_markers.clone();

        let theme_section = self.section(
            &palette,
            "Theme",
            "Choose how Missal looks. System follows your operating system.",
            h_flex()
                .gap_4()
                .children(ThemeChoice::ALL.map(|choice| self.theme_card(&palette, choice, theme))),
        );

        let marker_section = self.section(
            &palette,
            "Field markers",
            "The characters that surround a field name inside a document.",
            v_flex()
                .gap_3()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(div().w(px(72.)).child(self.marker_open.clone()))
                        .child(div().text_size(px(16.)).child(label("نام").rtl()))
                        .child(div().w(px(72.)).child(self.marker_close.clone()))
                        .child(div().w(px(16.)))
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .text_size(px(13.))
                                .text_color(palette.muted_foreground)
                                .child("Example")
                                .child(
                                    div()
                                        .px_2()
                                        .rounded(px(5.))
                                        .bg(palette.muted)
                                        .text_size(px(15.))
                                        .text_color(palette.field)
                                        .child(label(markers_preview.token("جرم")).rtl()),
                                ),
                        ),
                )
                .children(self.marker_error.then(|| {
                    div()
                        .text_size(px(12.))
                        .text_color(palette.destructive)
                        .child("Use 1–3 symbols, no letters or numbers.")
                }))
                .children(self.section_actions(
                    &palette,
                    markers_dirty,
                    self.saving[0],
                    "markers",
                    |this, window, cx| this.save_markers(window, cx),
                    None,
                    cx,
                )),
        );

        let globals_body: AnyElement = match &load {
            Load::Loading => v_flex()
                .gap_2()
                .children((0..4usize).map(|index| {
                    div()
                        .id(("global-skeleton", index))
                        .h(px(36.))
                        .rounded(px(7.))
                        .bg(palette.muted)
                }))
                .into_any_element(),
            Load::Failed(message) => v_flex()
                .gap_2()
                .child(
                    div()
                        .text_size(px(13.))
                        .child("Could not load global values"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(palette.muted_foreground)
                        .child(message.clone()),
                )
                .child(
                    Button::new("retry-settings")
                        .outline()
                        .small()
                        .label("Try again")
                        .on_click({
                            let catalog = self.models.catalog.clone();
                            move |_, _, cx| catalog.update(cx, |catalog, cx| catalog.reload(cx))
                        }),
                )
                .into_any_element(),
            Load::Ready => v_flex()
                .gap_2()
                .children(self.globals.iter().enumerate().map(|(index, row)| {
                    let is_new = row.id.is_none();
                    h_flex()
                        .id(("global-row", index))
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().min_w_0().child(row.name.clone()))
                        .child(div().flex_1().min_w_0().child(row.value.clone()))
                        .child(div().w(px(28.)).flex_shrink_0().children(is_new.then(|| {
                            Button::new(("remove-global", index))
                                .ghost()
                                .xsmall()
                                .icon(IconName::X)
                                .tooltip("Remove")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if index < this.globals.len()
                                        && this.globals[index].id.is_none()
                                    {
                                        this.globals.remove(index);
                                        cx.notify();
                                    }
                                }))
                        })))
                }))
                .child(
                    h_flex().child(
                        Button::new("add-global")
                            .outline()
                            .small()
                            .icon(IconName::Plus)
                            .label("Add value")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.add_global(window, cx)),
                            ),
                    ),
                )
                .children(self.section_actions(
                    &palette,
                    globals_dirty,
                    self.saving[1],
                    "globals",
                    |this, window, cx| this.save_globals(window, cx),
                    Some(|this, window, cx| this.reset_globals(window, cx)),
                    cx,
                ))
                .into_any_element(),
        };
        let globals_section = self.section(
            &palette,
            "Global values",
            "Shared values such as the police station name, filled into every FIR's documents.",
            globals_body,
        );

        let fields_body = v_flex()
            .gap_4()
            .child(
                h_flex().flex_wrap().gap_2().children(FIR_FIELDS.iter().map(|field| {
                    div()
                        .px_2()
                        .rounded(px(6.))
                        .bg(palette.muted)
                        .text_size(px(14.))
                        .text_color(palette.field)
                        .child(label(chips_markers.token(field.label)).rtl())
                })),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(palette.muted_foreground)
                    .child("Filled from each FIR. Add a custom field below; renaming keeps templates linked."),
            )
            .child(
                v_flex()
                    .gap_2()
                    .children(self.customs.iter().enumerate().map(|(index, row)| {
                        let is_new = row.id.is_none();
                        h_flex()
                            .id(("custom-row", index))
                            .items_center()
                            .gap_2()
                            .child(div().flex_1().min_w_0().child(row.name.clone()))
                            .child(div().w(px(28.)).flex_shrink_0().children(is_new.then(|| {
                                Button::new(("remove-custom", index))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::X)
                                    .tooltip("Remove")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if index < this.customs.len() && this.customs[index].id.is_none() {
                                            this.customs.remove(index);
                                            cx.notify();
                                        }
                                    }))
                            })))
                    }))
                    .child(
                        h_flex().child(
                            Button::new("add-custom")
                                .outline()
                                .small()
                                .icon(IconName::Plus)
                                .label("Add custom field")
                                .on_click(cx.listener(|this, _, window, cx| this.add_custom(window, cx))),
                        ),
                    ),
            )
            .children(self.section_actions(
                &palette,
                customs_dirty,
                self.saving[2],
                "customs",
                |this, window, cx| this.save_customs(window, cx),
                Some(|this, window, cx| this.reset_globals(window, cx)),
                cx,
            ));
        let fields_section = self.section(
            &palette,
            "FIR fields",
            "The built-in field names and your custom fields.",
            fields_body,
        );

        let dirty = markers_dirty || globals_dirty || customs_dirty;
        let _ = &self.subscriptions;
        v_flex()
            .id("settings")
            .key_context("Settings")
            .track_focus(&self.focus)
            .size_full()
            .on_action(cx.listener(|this, _: &Save, window, cx| this.save_all(window, cx)))
            .child(
                pane_header(&palette)
                    .child(div().text_size(px(14.)).font_medium().child("Settings"))
                    .child(div().flex_1())
                    .children(dirty.then(|| save_status(&palette, true))),
            )
            .child(
                div()
                    .id("settings-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        v_flex()
                            .max_w(px(896.))
                            .mx_auto()
                            .px_6()
                            .child(theme_section)
                            .child(marker_section)
                            .child(globals_section)
                            .child(fields_section)
                            .child(div().h(px(32.))),
                    ),
            )
    }
}
