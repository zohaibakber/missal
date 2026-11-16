use chrono::Datelike;
use gpui_kit::assets::IconName;
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::{WindowExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString,
    Styled, Subscription, Window, div, px,
};
use missal_library::LibraryEntry;
use missal_text::{LineHeight, label};

use crate::app::navigate;
use crate::library::{EntryStatus, LibraryModel};
use crate::state::{Load, Models, Route};
use crate::theme::{Palette, RADIUS};
use crate::ui::dialogs::{
    Choice, Tone, confirm_delete, dialog_footer, dialog_surface, dialog_title, open_choice,
    optical_top,
};
use crate::ui::{
    button, empty_state_body, ghost_sm, icon_button, isolate_urdu, primary_sm, skeleton_bar,
};

const WIDTH: f32 = 560.;
const LIST_HEIGHT: f32 = 360.;
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn published_on(millis: i64) -> Option<String> {
    let date = chrono::DateTime::from_timestamp_millis(millis)?.date_naive();
    let month = MONTHS.get(date.month0() as usize)?;
    Some(format!("{} {month} {}", date.day(), date.year()))
}

fn meta(entry: &LibraryEntry) -> String {
    let fields = if entry.field_count == 1 {
        "1 field".to_owned()
    } else {
        format!("{} fields", entry.field_count)
    };
    match published_on(entry.updated_at) {
        Some(date) => format!("{fields} · updated {date}"),
        None => fields,
    }
}

pub fn open_library(window: &mut Window, cx: &mut App) {
    let Some(model) = LibraryModel::get(cx) else {
        return;
    };
    model.update(cx, |model, cx| model.refresh(cx));
    let panel = cx.new(|cx| LibraryPanel::new(model, cx));
    window.open_dialog(cx, move |dialog: Dialog, window, cx| {
        let palette = *Palette::get(cx);
        dialog_surface(dialog, &palette, WIDTH)
            .margin_top(optical_top(window, LIST_HEIGHT + 140.))
            .child(panel.clone())
    });
}

struct LibraryPanel {
    model: Entity<LibraryModel>,
    _subscriptions: Vec<Subscription>,
}

impl LibraryPanel {
    fn new(model: Entity<LibraryModel>, cx: &mut Context<Self>) -> LibraryPanel {
        let templates = Models::get(cx).templates;
        LibraryPanel {
            _subscriptions: vec![
                cx.observe(&model, |_, _, cx| cx.notify()),
                cx.observe(&templates, |_, _, cx| cx.notify()),
            ],
            model,
        }
    }

    fn action(&self, index: usize, entry: &LibraryEntry, cx: &App) -> AnyElement {
        let model = self.model.read(cx);
        let id = entry.id.clone();
        let name = entry.name.clone();
        let handle = self.model.clone();
        match model.status(entry, cx) {
            EntryStatus::Busy => button(("library-busy", index), "Working…")
                .outline()
                .sm()
                .disabled(true)
                .into_any_element(),
            EntryStatus::Available => primary_sm(("library-add", index), "Add")
                .on_click(move |_, window, cx| {
                    handle.update(cx, |model, cx| model.install_entry(&id, window, cx));
                })
                .into_any_element(),
            EntryStatus::UpdateAvailable { edited } => {
                primary_sm(("library-update", index), "Update")
                    .on_click(move |_, window, cx| {
                        let handle = handle.clone();
                        let id = id.clone();
                        if !edited {
                            handle.update(cx, |model, cx| model.install_entry(&id, window, cx));
                            return;
                        }
                        open_choice(
                            window,
                            cx,
                            format!("Replace your changes to «{}»?", isolate_urdu(&name)),
                            "Your edits to this template will be replaced by the library version. Documents already added to FIRs are not changed.",
                            vec![
                                Choice::new("keep", "Keep my version", Tone::Neutral, |_, _| {}),
                                Choice::new("replace", "Replace", Tone::Primary, move |window, cx| {
                                    handle.update(cx, |model, cx| {
                                        model.install_entry(&id, window, cx)
                                    });
                                }),
                            ],
                        );
                    })
                    .into_any_element()
            }
            EntryStatus::Installed => {
                let template = model.link_for_entry(&entry.id).map(|link| link.template_id);
                ghost_sm(("library-open", index), "Open")
                    .disabled(template.is_none())
                    .on_click(move |_, window, cx| {
                        if let Some(template) = template {
                            window.close_dialog(cx);
                            navigate(Route::Template(template), window, cx);
                        }
                    })
                    .into_any_element()
            }
        }
    }

    fn remove(&self, index: usize, entry: &LibraryEntry) -> AnyElement {
        let handle = self.model.clone();
        let id = entry.id.clone();
        let name = entry.name.clone();
        icon_button(("library-remove", index), IconName::Trash, "Remove from library")
            .dense()
            .on_click(move |_, window, cx| {
                let handle = handle.clone();
                let id = id.clone();
                confirm_delete(
                    window,
                    cx,
                    format!("Remove «{}» from the library?", isolate_urdu(&name)),
                    "Other people can no longer add it. Copies they already added stay on their computers.",
                    "Remove",
                    move |window, cx| {
                        handle.update(cx, |model, cx| model.unpublish(&id, window, cx));
                    },
                );
            })
            .into_any_element()
    }

    fn row(&self, index: usize, entry: &LibraryEntry, palette: &Palette, cx: &App) -> AnyElement {
        let publisher = self.model.read(cx).publisher;
        h_flex()
            .id(("library-row", index))
            .min_h(px(56.))
            .px(px(10.))
            .py(px(6.))
            .gap(px(12.))
            .items_center()
            .rounded(px(RADIUS))
            .hover(|style| style.bg(palette.hover))
            .child(
                div()
                    .w(px(84.))
                    .flex_none()
                    .child(self.action(index, entry, cx)),
            )
            .children(publisher.then(|| self.remove(index, entry)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .items_end()
                    .child(
                        label(entry.name.clone())
                            .text_size(px(16.))
                            .text_color(palette.ink)
                            .rtl()
                            .leading(LineHeight::Px(28.))
                            .truncate(),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(palette.ink_faint)
                            .font_features(crate::ui::tabular())
                            .child(meta(entry)),
                    ),
            )
            .into_any_element()
    }

    fn body(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let model = self.model.read(cx);
        let refresh = {
            let handle = self.model.clone();
            move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
                handle.update(cx, |model, cx| model.refresh(cx));
            }
        };
        match &model.load {
            Load::Loading => v_flex()
                .gap(px(10.))
                .pt(px(8.))
                .children((0..5).map(|_| skeleton_bar(WIDTH - 40., 40.)))
                .into_any_element(),
            Load::Failed(message) => div()
                .pt(px(40.))
                .child(empty_state_body(IconName::TriangleAlert,
                    "Could not open the library",
                    Some(SharedString::from(message.clone())),
                    Some(
                        button("library-retry", "Try again")
                            .outline()
                            .sm()
                            .on_click(refresh)
                            .into_any_element(),
                    ),
                ))
                .into_any_element(),
            Load::Ready if model.entries.is_empty() => div()
                .pt(px(48.))
                .child(empty_state_body(IconName::Library,
                    "Nothing has been published yet",
                    model.publisher.then(|| {
                        SharedString::from(
                            "Open a template and choose Publish to library from its More actions menu.",
                        )
                    }),
                    None,
                ))
                .into_any_element(),
            Load::Ready => {
                let entries = model.entries.clone();
                v_flex()
                    .id("library-list")
                    .overflow_y_scroll()
                    .h_full()
                    .gap(px(2.))
                    .children(
                        entries
                            .iter()
                            .enumerate()
                            .map(|(index, entry)| self.row(index, entry, palette, cx)),
                    )
                    .into_any_element()
            }
        }
    }
}

impl Render for LibraryPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let model = self.model.read(cx);
        let fetching = model.is_fetching();
        let subtitle = if model.publisher {
            "Templates you publish appear here for everyone who uses Missal."
        } else {
            "Templates published for everyone who uses Missal."
        };
        let handle = self.model.clone();
        v_flex()
            .child(
                h_flex()
                    .items_center()
                    .child(dialog_title("Template library"))
                    .child(div().flex_1())
                    .child(
                        icon_button("library-refresh", IconName::RefreshCw, "Refresh")
                            .disabled(fetching)
                            .on_click(move |_, _, cx| {
                                handle.update(cx, |model, cx| model.refresh(cx));
                            }),
                    ),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .text_color(palette.ink_muted)
                    .child(label(subtitle).leading(LineHeight::Px(20.))),
            )
            .child(
                div()
                    .mt(px(12.))
                    .h(px(LIST_HEIGHT))
                    .child(self.body(&palette, cx)),
            )
            .child(
                dialog_footer().child(div().flex_1()).child(
                    button("library-close", "Close")
                        .primary()
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
            )
    }
}
