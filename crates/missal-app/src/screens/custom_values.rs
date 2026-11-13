use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Sizable, StyledExt, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::component::Disableable as _;

use gpui_kit::{
    App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    Styled, Subscription, Window, div, px,
};
use missal_core::{FirId, Placeholder, PlaceholderId, PlaceholderValue};
use missal_text::{TextInput, TextInputEvent, label};

use crate::state::rows::fold_for_search;
use crate::state::{Models, run_db_in, toast_error, toast_success};
use crate::theme::Palette;
use crate::ui::dirty_dot;
use crate::ui::input::{Kind, text_input};

type Saved = Rc<dyn Fn(Vec<PlaceholderValue>, &mut Window, &mut App)>;

struct ValueRow {
    placeholder: Placeholder,
    input: Entity<TextInput>,
    initial: String,
}

pub struct CustomValues {
    fir_id: FirId,
    rows: Vec<ValueRow>,
    search: Entity<TextInput>,
    query: String,
    saving: bool,
    on_saved: Saved,
    _subscriptions: Vec<Subscription>,
}

impl CustomValues {
    pub fn new(
        fir_id: FirId,
        fields: Vec<Placeholder>,
        existing: &[PlaceholderValue],
        on_saved: Saved,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> CustomValues {
        let search = text_input(window, cx, Kind::Urdu, "Search custom fields", "");
        let mut subscriptions = vec![cx.subscribe(&search, |this, input, event: &TextInputEvent, cx| {
            if *event == TextInputEvent::Change {
                this.query = input.read(cx).text().to_owned();
                cx.notify();
            }
        })];
        let rows = fields
            .into_iter()
            .map(|placeholder| {
                let initial = existing
                    .iter()
                    .find(|value| value.placeholder_id == placeholder.id)
                    .map(|value| value.value.clone())
                    .unwrap_or_default();
                let input = text_input(window, cx, Kind::Urdu, "", &initial);
                subscriptions.push(cx.subscribe(&input, |_, _, event: &TextInputEvent, cx| {
                    if *event == TextInputEvent::Change {
                        cx.notify();
                    }
                }));
                ValueRow { placeholder, input, initial }
            })
            .collect();
        CustomValues {
            fir_id,
            rows,
            search,
            query: String::new(),
            saving: false,
            on_saved,
            _subscriptions: subscriptions,
        }
    }

    pub fn inputs(&self) -> Vec<(PlaceholderId, Entity<TextInput>)> {
        self.rows.iter().map(|row| (row.placeholder.id, row.input.clone())).collect()
    }

    pub fn is_dirty(&self, cx: &App) -> bool {
        self.rows.iter().any(|row| row.input.read(cx).text() != row.initial)
    }

    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let mut upserts: Vec<(PlaceholderId, String)> = Vec::new();
        let mut removals: Vec<PlaceholderId> = Vec::new();
        for row in &self.rows {
            let current = row.input.read(cx).text().to_owned();
            if current == row.initial {
                continue;
            }
            if current.trim().is_empty() {
                if !row.initial.is_empty() {
                    removals.push(row.placeholder.id);
                }
            } else {
                upserts.push((row.placeholder.id, current));
            }
        }
        if upserts.is_empty() && removals.is_empty() {
            return;
        }
        self.saving = true;
        cx.notify();
        let fir_id = self.fir_id;
        let on_saved = self.on_saved.clone();
        run_db_in(
            window,
            cx,
            move |store| {
                if !upserts.is_empty() {
                    store.set_placeholder_values(fir_id, &upserts)?;
                }
                for id in removals {
                    store.remove_placeholder_value(fir_id, id)?;
                }
                store.placeholder_values(fir_id)
            },
            move |this, result, window, cx| {
                this.saving = false;
                match result {
                    Ok(values) => {
                        for row in &mut this.rows {
                            row.initial = values
                                .iter()
                                .find(|value| value.placeholder_id == row.placeholder.id)
                                .map(|value| value.value.clone())
                                .unwrap_or_default();
                        }
                        window.close_sheet(cx);
                        toast_success(window, cx, "Custom values saved");
                        on_saved(values, window, cx);
                    }
                    Err(error) => toast_error(window, cx, error.message()),
                }
                cx.notify();
            },
        );
    }
}

impl Render for CustomValues {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let words: Vec<String> = fold_for_search(&self.query).split_whitespace().map(str::to_owned).collect();
        let rows: Vec<_> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                let haystack = fold_for_search(&row.placeholder.label);
                words.iter().all(|word| haystack.contains(word.as_str()))
            })
            .collect();
        let empty = rows.is_empty();
        v_flex()
            .id("custom-values")
            .on_action(cx.listener(|this, _: &crate::shortcuts::Save, window, cx| this.save(window, cx)))
            .px_4()
            .pb_4()
            .gap_3()
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(palette.muted_foreground)
                    .child("Values for custom fields in this FIR. Documents show them in place of the field name."),
            )
            .child(self.search.clone())
            .child(v_flex().gap_3().children(rows.into_iter().map(|(index, row)| {
                v_flex()
                    .id(("custom-value-row", index))
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .flex_row_reverse()
                            .text_size(px(14.))
                            .child(label(row.placeholder.label.clone()).rtl()),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .child(div().flex_1().min_w_0().child(row.input.clone()))
                            .child({
                                let input = row.input.clone();
                                Button::new(("clear-custom-value", index))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::X)
                                    .tooltip("Clear value")
                                    .on_click(move |_, _, cx| input.update(cx, |input, cx| input.set_text("", cx)))
                            }),
                    )
            })))
            .when(empty, |this| {
                this.child(
                    div()
                        .py_4()
                        .text_size(px(13.))
                        .text_color(palette.muted_foreground)
                        .child("No matching custom fields."),
                )
            })
    }
}

pub struct CustomValuesFooter {
    values: Entity<CustomValues>,
    _subscription: Subscription,
}

impl Render for CustomValuesFooter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let dirty = self.values.read(cx).is_dirty(cx);
        let saving = self.values.read(cx).saving;
        let values = self.values.clone();
        h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .gap_1p5()
                    .text_size(px(12.))
                    .text_color(palette.muted_foreground)
                    .child(dirty_dot(&palette, dirty))
                    .child(if dirty { "Unsaved changes" } else { "No changes" }),
            )
            .child(div().flex_1())
            .child(
                Button::new("custom-values-save")
                    .primary()
                    .small()
                    .loading(saving)
                    .disabled(!dirty)
                    .label("Save values")
                    .on_click(move |_, window, cx| values.update(cx, |values, cx| values.save(window, cx))),
            )
    }
}

pub fn open_custom_values(
    fir_id: FirId,
    fir_no: String,
    existing: Vec<PlaceholderValue>,
    on_saved: Saved,
    window: &mut Window,
    cx: &mut App,
) {
    let fields: Vec<Placeholder> = Models::get(cx).catalog.read(cx).custom_fields().cloned().collect();
    let values = cx.new(|cx| CustomValues::new(fir_id, fields, &existing, on_saved, window, cx));
    let footer = {
        let values = values.clone();
        cx.new(|cx| CustomValuesFooter {
            _subscription: cx.observe(&values, |_, _, cx| cx.notify()),
            values,
        })
    };
    window.open_sheet(cx, move |sheet, _, _| {
        sheet
            .size(px(520.))
            .title(div().text_size(px(15.)).font_semibold().child(missal_text::label(format!("Custom values · FIR {fir_no}"))))
            .child(values.clone())
            .footer(footer.clone())
    });
}
