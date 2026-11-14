use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::{WindowExt, h_flex, v_flex};
use gpui_kit::{
    App, AppContext, Context, Entity, FontWeight, Global, IntoElement, ParentElement, Render,
    Styled, Subscription, Window, div, px,
};
use missal_text::{TextInput, TextInputEvent};

use crate::shortcuts::{Group, SHORTCUTS, Shortcut};
use crate::state::rows::fold_for_search;
use crate::theme::{CHEATSHEET_WIDTH, Palette};
use crate::ui::dialogs::{dialog_surface, dialog_title};
use crate::ui::input::{Kind, field, field_input};
use crate::ui::keycaps_chord;

pub struct CheatsheetOpen(pub bool);

impl Global for CheatsheetOpen {}

pub struct Cheatsheet {
    filter: Entity<TextInput>,
    _subscription: Subscription,
}

impl Cheatsheet {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Cheatsheet {
        let filter = field_input(window, cx, Kind::Search, "Filter shortcuts", "");
        let subscription = cx.subscribe(&filter, |_, _, event: &TextInputEvent, cx| {
            if *event == TextInputEvent::Change {
                cx.notify();
            }
        });
        Cheatsheet {
            filter,
            _subscription: subscription,
        }
    }

    pub fn filter(&self) -> &Entity<TextInput> {
        &self.filter
    }

    pub fn visible(&self, cx: &App) -> Vec<&'static Shortcut> {
        let query = fold_for_search(self.filter.read(cx).text());
        let words: Vec<&str> = query.split_whitespace().collect();
        SHORTCUTS
            .iter()
            .filter(|shortcut| {
                let haystack = fold_for_search(&format!(
                    "{} {} {}",
                    shortcut.label,
                    shortcut.keys,
                    shortcut.group.title()
                ));
                words.iter().all(|word| haystack.contains(word))
            })
            .collect()
    }
}

impl Render for Cheatsheet {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let visible = self.visible(cx);
        let group = |group: Group| {
            let rows: Vec<_> = visible
                .iter()
                .filter(|shortcut| shortcut.group == group)
                .collect();
            (!rows.is_empty()).then(|| {
                v_flex()
                    .child(
                        div()
                            .pb(px(4.))
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.ink_muted)
                            .child(group.title()),
                    )
                    .children(rows.into_iter().map(|shortcut| {
                        h_flex()
                            .h(px(28.))
                            .items_center()
                            .justify_between()
                            .gap(px(12.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(palette.ink)
                                    .child(shortcut.label),
                            )
                            .child(keycaps_chord(&palette, shortcut.keys))
                    }))
            })
        };
        let column = |groups: &[Group]| {
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(16.))
                .children(groups.iter().filter_map(|g| group(*g)))
        };
        v_flex()
            .gap(px(16.))
            .child(field("cheatsheet-filter", &self.filter, Kind::Search).width(px(280.)))
            .child(
                h_flex()
                    .items_start()
                    .gap(px(32.))
                    .child(column(&[Group::General, Group::FirList, Group::Templates]))
                    .child(column(&[Group::Workspace, Group::Editor, Group::Print])),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(palette.ink_faint)
                    .child("Single-key shortcuts work when no text field or document is focused. Press Esc to leave one."),
            )
    }
}

pub fn is_open(cx: &App) -> bool {
    cx.try_global::<CheatsheetOpen>().is_some_and(|open| open.0)
}

pub fn open(window: &mut Window, cx: &mut App) {
    if is_open(cx) && window.has_active_dialog(cx) {
        window.close_dialog(cx);
        cx.set_global(CheatsheetOpen(false));
        return;
    }
    if window.has_active_dialog(cx) {
        return;
    }
    let sheet = cx.new(|cx| Cheatsheet::new(window, cx));
    cx.set_global(CheatsheetOpen(true));
    window.open_dialog(cx, move |dialog: Dialog, _, cx| {
        let palette = *Palette::get(cx);
        dialog_surface(dialog, &palette, CHEATSHEET_WIDTH)
            .margin_top(px(56.))
            .keyboard(true)
            .on_close(|_, _, cx| cx.set_global(CheatsheetOpen(false)))
            .child(
                v_flex()
                    .gap(px(12.))
                    .child(dialog_title("Keyboard shortcuts"))
                    .child(sheet.clone()),
            )
    });
}
