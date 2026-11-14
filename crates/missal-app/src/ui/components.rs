use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Context, Div, Entity, FocusHandle, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Window, div, px,
};
use missal_core::FirStatus;
use missal_text::TextInput;

use crate::state::ToastKind;
use crate::theme::{Palette, RADIUS_XL};
use crate::ui::dialogs::{dialog_body, dialog_title};
use crate::ui::input::{Kind, field, field_input};
use crate::ui::menu::{menu_item, menu_panel, menu_separator};
use crate::ui::status::status_label;
use crate::ui::titlebar::{HeaderContent, HeaderSource};
use crate::ui::toast::{ToastSpec, toast_card};
use crate::ui::{
    ButtonKind, KeyTone, UrRole, button, caption, checkbox, empty_state_body, icon_button, keycap,
    keycaps_toned, sized_icon, ur_inline,
};

pub struct ComponentsScreen {
    focus: FocusHandle,
    date: Entity<TextInput>,
    fir_no: Entity<TextInput>,
    urdu: Entity<TextInput>,
    error: Entity<TextInput>,
    search: Entity<TextInput>,
}

impl ComponentsScreen {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> ComponentsScreen {
        ComponentsScreen {
            focus: cx.focus_handle(),
            date: field_input(window, cx, Kind::Date, "DD-MM-YYYY", ""),
            fir_no: field_input(window, cx, Kind::LatinNumeric, "", "412/2026"),
            urdu: field_input(window, cx, Kind::Urdu, "", "انسپکٹر ملک ارشد"),
            error: field_input(window, cx, Kind::Urdu, "", ""),
            search: field_input(window, cx, Kind::SearchRtl, "تلاش", "ریمانڈ"),
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }
}

impl HeaderSource for ComponentsScreen {
    fn header(&mut self, _: &mut Window, _: &mut Context<Self>) -> HeaderContent {
        HeaderContent::new("Components")
    }
}

fn line() -> Div {
    h_flex().items_center().gap(px(8.)).flex_wrap()
}

fn cell(palette: &Palette, right: bool) -> Div {
    v_flex()
        .flex_1()
        .min_w_0()
        .h_full()
        .pt(px(20.))
        .px(px(24.))
        .when(right, |this| this.border_r_1().border_color(palette.line))
}

fn cap(palette: &Palette, text: &'static str, top: f32) -> Div {
    caption(palette, text)
        .line_height(px(20.))
        .mt(px(top))
        .mb(px(12.))
}

impl ComponentsScreen {
    fn buttons(&self, palette: &Palette) -> Div {
        cell(palette, true)
            .child(cap(palette, "Buttons · 32 / 28 px", 0.))
            .child(
                line()
                    .child(button("c-create", "Create FIR").primary())
                    .child(button("c-hover", "Hover").primary().show_hover())
                    .child(button("c-focus", "Focus").primary().show_focus())
                    .child(button("c-disabled", "Disabled").primary().disabled(true)),
            )
            .child(
                line()
                    .mt(px(10.))
                    .child(button("c-import", "Import").outline())
                    .child(button("c-outline-hover", "Hover").outline().show_hover())
                    .child(button("c-cancel", "Cancel").ghost())
                    .child(button("c-delete", "Delete FIR").danger()),
            )
            .child(
                line()
                    .mt(px(10.))
                    .child(
                        button("c-small", "Small")
                            .outline()
                            .sm()
                            .icon(IconName::FileUp),
                    )
                    .child(
                        button("c-ghost", "Ghost")
                            .kind(ButtonKind::GhostMuted)
                            .sm()
                            .icon(IconName::Columns3),
                    )
                    .child(icon_button("c-more", IconName::Ellipsis, "More actions"))
                    .child(
                        icon_button("c-bold", IconName::Bold, "Bold")
                            .keys("Mod+B")
                            .toggled(true),
                    )
                    .child(
                        icon_button("c-redo", IconName::Redo2, "Redo")
                            .keys("Mod+Y")
                            .disabled(true),
                    ),
            )
            .child(cap(palette, "Inputs · Latin 32, Urdu 36", 18.))
            .child(
                line()
                    .child(field("c-date", &self.date, Kind::Date).width(px(150.)))
                    .child(
                        field("c-fir-no", &self.fir_no, Kind::LatinNumeric)
                            .width(px(150.))
                            .show_focus(),
                    ),
            )
            .child(
                line()
                    .mt(px(10.))
                    .child(field("c-urdu", &self.urdu, Kind::Urdu).width(px(150.)))
                    .child(
                        field("c-error", &self.error, Kind::Urdu)
                            .width(px(150.))
                            .error(true),
                    ),
            )
            .child(
                div()
                    .mt(px(6.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(palette.danger_text)
                    .child("Enter the FIR number, for example 412/2026."),
            )
    }

    fn dialog_and_toasts(&self, palette: &Palette) -> Div {
        let toast = |spec: ToastSpec, action: &'static str| {
            div().child(
                toast_card(palette, &spec).child(
                    div()
                        .flex_none()
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.accent)
                        .child(action),
                ),
            )
        };
        cell(palette, true)
            .child(cap(palette, "Confirm dialog · 420 px", 0.))
            .child(
                v_flex()
                    .w_full()
                    .p(px(20.))
                    .rounded(px(RADIUS_XL))
                    .bg(palette.raised)
                    .shadow(palette.shadow_lg())
                    .child(dialog_title("Delete FIR 9/2026?"))
                    .child(dialog_body(
                        palette,
                        "Its 3 documents are deleted too. This can't be undone.",
                    ))
                    .child(
                        h_flex()
                            .mt(px(18.))
                            .gap(px(8.))
                            .justify_end()
                            .child(button("c-dialog-cancel", "Cancel").outline().show_focus())
                            .child(button("c-dialog-delete", "Delete FIR").danger()),
                    ),
            )
            .child(cap(palette, "Toasts · bottom right, 4 s", 22.))
            .child(
                v_flex()
                    .gap(px(8.))
                    .child(toast(
                        ToastSpec::new(
                            ToastKind::Success,
                            format!(
                                "Added {}",
                                crate::ui::isolate_urdu("ضمنی حصول ریمانڈ جسمانی")
                            ),
                        ),
                        "Undo",
                    ))
                    .child(toast(
                        ToastSpec::new(ToastKind::Error, "Couldn't save. The disk is full."),
                        "Retry",
                    )),
            )
    }

    fn add_document(&self, palette: &Palette) -> Div {
        let row = |id: &'static str, name: &'static str, checked: bool, added: bool, hl: bool| {
            h_flex()
                .id(id)
                .h(px(40.))
                .mx(px(4.))
                .px(px(10.))
                .gap(px(10.))
                .flex_row_reverse()
                .items_center()
                .rounded(px(6.))
                .when(hl, |this| this.bg(palette.hover))
                .child(checkbox(palette, (id, 1usize), checked))
                .child(
                    div().flex_1().min_w_0().text_color(palette.ink).child(
                        ur_inline(name, UrRole::Body)
                            .align(missal_text::Align::Right)
                            .truncate(),
                    ),
                )
                .when(added, |this| {
                    this.child(
                        div()
                            .flex_none()
                            .text_size(px(12.))
                            .text_color(palette.ink_faint)
                            .child("Added"),
                    )
                })
        };
        cell(palette, false)
            .child(cap(palette, "Add document · popover 360 px", 0.))
            .child(
                v_flex()
                    .w_full()
                    .rounded(px(8.))
                    .bg(palette.raised)
                    .shadow(palette.shadow_md())
                    .child(
                        h_flex()
                            .h(px(44.))
                            .px(px(12.))
                            .gap(px(8.))
                            .items_center()
                            .border_b_1()
                            .border_color(palette.line)
                            .child(keycap(palette, "Esc"))
                            .child(div().flex_1().min_w_0().child(self.search.clone()))
                            .child(sized_icon(IconName::Search, 14., palette.ink_faint)),
                    )
                    .child(div().h(px(4.)))
                    .child(row("c-add-1", "ضمنی حصول ریمانڈ جسمانی", true, true, true))
                    .child(row(
                        "c-add-2",
                        "چالان انویسٹی گیشن 401 ملزمان جوڈیشل",
                        true,
                        false,
                        false,
                    ))
                    .child(row(
                        "c-add-3",
                        "چالان منشیات ملزم جوڈیشل",
                        false,
                        false,
                        false,
                    ))
                    .child(
                        h_flex()
                            .mt(px(4.))
                            .h(px(44.))
                            .pl(px(12.))
                            .pr(px(8.))
                            .gap(px(8.))
                            .items_center()
                            .border_t_1()
                            .border_color(palette.line)
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(12.))
                                    .text_color(palette.ink_muted)
                                    .child("1 selected"),
                            )
                            .child(
                                button("c-add-confirm", "Add 1 document")
                                    .primary()
                                    .sm()
                                    .keys("Mod+Enter"),
                            ),
                    ),
            )
    }

    fn menu(&self, palette: &Palette) -> Div {
        cell(palette, true)
            .child(cap(
                palette,
                "Menu · items 32 px, Urdu left-aligned in LTR menus",
                0.,
            ))
            .child(
                menu_panel(palette)
                    .w(px(220.))
                    .child(
                        menu_item("c-menu-open", "Open")
                            .icon(IconName::FolderOpen)
                            .keys("Enter"),
                    )
                    .child(
                        menu_item("c-menu-edit", "Edit details")
                            .icon(IconName::Pencil)
                            .keys("E")
                            .highlighted(true),
                    )
                    .child(menu_item("c-menu-print", "Print…").icon(IconName::Printer))
                    .child(menu_separator(palette))
                    .child(
                        menu_item("c-menu-delete", "Delete FIR")
                            .icon(IconName::Trash)
                            .keys("Delete")
                            .danger(),
                    ),
            )
    }

    fn tooltip_focus_status(&self, palette: &Palette) -> Div {
        let ring = |inner: AnyElement, radius: f32| {
            div()
                .flex_none()
                .rounded(px(radius))
                .shadow(palette.focus_ring())
                .child(inner)
        };
        cell(palette, true)
            .child(cap(palette, "Tooltip · 400 ms delay, no animation", 0.))
            .child(
                h_flex().child(
                    h_flex()
                        .items_center()
                        .gap(px(8.))
                        .py(px(4.))
                        .px(px(8.))
                        .rounded(px(6.))
                        .bg(palette.ink)
                        .text_color(palette.surface)
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .child("Print")
                        .child(keycaps_toned(palette, "Mod+P", KeyTone::OnTooltip)),
                ),
            )
            .child(cap(palette, "Focus ring · 2 px ring, 2 px gap", 22.))
            .child(
                line()
                    .child(ring(
                        checkbox(palette, "c-check", false).into_any_element(),
                        4.,
                    ))
                    .child(div().text_size(px(13.)).child("Checkbox"))
                    .child(icon_button("c-plus", IconName::Plus, "Add").show_focus())
                    .child(div().text_size(px(13.)).child("Icon button")),
            )
            .child(cap(palette, "Status", 22.))
            .child(
                line().children(
                    FirStatus::ALL
                        .into_iter()
                        .map(|status| status_label(palette, status)),
                ),
            )
    }

    fn empty(&self, palette: &Palette) -> Div {
        cell(palette, false)
            .child(cap(palette, "Empty state", 0.))
            .child(
                v_flex()
                    .items_center()
                    .pt(px(18.))
                    .pb(px(6.))
                    .child(empty_state_body(
                        palette,
                        IconName::FileText,
                        "No documents yet",
                        Some("Add a template to start writing for this FIR.".into()),
                        Some(
                            button("c-empty-add", "Add document")
                                .primary()
                                .sm()
                                .icon(IconName::Plus)
                                .keys("A")
                                .into_any_element(),
                        ),
                    )),
            )
    }
}

impl Render for ComponentsScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        v_flex()
            .id("components-sheet")
            .track_focus(&self.focus)
            .size_full()
            .font_family(missal_text::UI_FAMILY)
            .text_size(px(14.))
            .text_color(palette.ink)
            .child(
                h_flex()
                    .flex_none()
                    .h(px(421.))
                    .items_start()
                    .border_b_1()
                    .border_color(palette.line)
                    .child(self.buttons(&palette))
                    .child(self.dialog_and_toasts(&palette))
                    .child(self.add_document(&palette)),
            )
            .child(
                h_flex()
                    .flex_1()
                    .items_start()
                    .child(self.menu(&palette))
                    .child(self.tooltip_focus_status(&palette))
                    .child(self.empty(&palette)),
            )
    }
}
