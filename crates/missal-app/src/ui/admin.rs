use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::{StyledExt as _, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div, px,
};
use missal_text::label;

use crate::auth::AuthModel;
use crate::theme::{PAGE_GUTTER, Palette, RADIUS_XL};
use crate::ui::dialogs::Tone;
use crate::ui::part::{Part, part};
use crate::ui::{ButtonKind, button, empty_state, icon_button};

pub const PANE_WIDTH: f32 = 380.;
pub const PANE_PAD: f32 = 20.;
pub const TOOLBAR_HEIGHT: f32 = 52.;
pub const ROW_HEIGHT: f32 = 52.;
pub const ACTIONS_WIDTH: f32 = 44.;
pub const FACT_LABEL: f32 = 116.;

pub fn is_admin(cx: &App) -> bool {
    AuthModel::get(cx).is_some_and(|auth| {
        auth.read(cx)
            .account()
            .is_some_and(|account| account.role.is_admin())
    })
}

pub fn admins_only() -> AnyElement {
    empty_state(
        IconName::ShieldAlert,
        "Only administrators can open this page",
        Some("Ask an administrator if you need access.".into()),
        None,
    )
    .into_any_element()
}

pub type Action = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone)]
pub struct Confirm {
    pub key: SharedString,
    pub id: &'static str,
    pub title: SharedString,
    pub body: SharedString,
    pub label: SharedString,
    pub tone: Tone,
    pub action: Action,
}

impl Confirm {
    pub fn new(
        key: impl Into<SharedString>,
        id: &'static str,
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        label: impl Into<SharedString>,
        tone: Tone,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Confirm {
        Confirm {
            key: key.into(),
            id,
            title: title.into(),
            body: body.into(),
            label: label.into(),
            tone,
            action: Rc::new(action),
        }
    }
}

pub fn confirm_card(
    confirm: &Confirm,
    on_cancel: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let danger = confirm.tone == Tone::Danger;
    let action = confirm.action.clone();
    let cancel = Rc::new(on_cancel);
    let after = cancel.clone();
    part(move |base, palette| {
        let tint = if danger { palette.danger } else { palette.ring };
        base.v_flex()
            .gap(px(4.))
            .p(px(14.))
            .rounded(px(RADIUS_XL))
            .border_1()
            .border_color(tint.opacity(0.36))
            .bg(tint.opacity(if palette.dark { 0.08 } else { 0.04 }))
            .text_size(px(14.))
            .line_height(px(20.))
    })
    .id(ElementId::Name(format!("{}-card", confirm.id).into()))
    .child(
        div()
            .font_weight(FontWeight::MEDIUM)
            .child(label(confirm.title.clone()).ltr()),
    )
    .child(muted(confirm.body.clone()))
    .child(
        h_flex()
            .mt(px(10.))
            .gap(px(8.))
            .justify_end()
            .child(
                button(
                    ElementId::Name(format!("{}-cancel", confirm.id).into()),
                    "Cancel",
                )
                .outline()
                .sm()
                .on_click(move |_, window, cx| cancel(window, cx)),
            )
            .child(
                button(confirm.id, confirm.label.clone())
                    .kind(if danger {
                        ButtonKind::Danger
                    } else {
                        ButtonKind::Primary
                    })
                    .sm()
                    .on_click(move |_, window, cx| {
                        after(window, cx);
                        action(window, cx);
                    }),
            ),
    )
    .into_any_element()
}

fn muted(text: SharedString) -> Part {
    part(|base, palette| base.text_color(palette.ink_muted)).child(label(text).ltr())
}

pub fn page_toolbar() -> Part {
    part(|base, _| {
        base.h_flex()
            .flex_none()
            .h(px(TOOLBAR_HEIGHT))
            .px(px(PAGE_GUTTER))
            .gap(px(12.))
            .items_center()
    })
}

pub fn pane() -> Part {
    part(|base, palette| {
        base.v_flex()
            .flex_none()
            .w(px(PANE_WIDTH))
            .h_full()
            .border_l_1()
            .border_color(palette.line)
            .bg(palette.surface)
    })
}

pub fn pane_header(
    title: impl IntoElement,
    subtitle: Option<AnyElement>,
    close_id: &'static str,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    h_flex()
        .flex_none()
        .px(px(PANE_PAD))
        .pt(px(16.))
        .pb(px(14.))
        .gap(px(8.))
        .items_start()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(4.))
                .child(title)
                .children(subtitle),
        )
        .child(
            icon_button(close_id, IconName::X, "Close")
                .keys("Escape")
                .on_click(move |_, window, cx| on_close(window, cx)),
        )
        .into_any_element()
}

pub fn pane_title(text: impl Into<SharedString>) -> Part {
    part(|base, palette| {
        base.min_w_0()
            .text_size(px(18.))
            .line_height(px(26.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(palette.ink)
    })
    .child(label(text.into()).ltr().truncate())
}

pub fn pane_section(title: &'static str) -> Part {
    part(|base, palette| {
        base.v_flex()
            .gap(px(10.))
            .px(px(PANE_PAD))
            .py(px(16.))
            .border_t_1()
            .border_color(palette.line)
    })
    .child(
        part(|base, palette| {
            base.text_size(px(12.))
                .line_height(px(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.ink_muted)
        })
        .child(title),
    )
}

pub fn pane_footer() -> Part {
    part(|base, palette| {
        base.h_flex()
            .flex_none()
            .px(px(PANE_PAD))
            .py(px(12.))
            .gap(px(8.))
            .justify_end()
            .border_t_1()
            .border_color(palette.line)
    })
}

pub fn fact(name: &'static str, value: impl IntoElement, palette: &Palette) -> AnyElement {
    h_flex()
        .min_h(px(28.))
        .gap(px(8.))
        .items_center()
        .text_size(px(13.))
        .line_height(px(18.))
        .child(
            div()
                .w(px(FACT_LABEL))
                .flex_none()
                .text_color(palette.ink_muted)
                .child(name),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_color(palette.ink)
                .font_features(crate::ui::tabular())
                .child(value),
        )
        .into_any_element()
}

pub fn fact_text(value: impl Into<SharedString>) -> AnyElement {
    label(value.into()).ltr().truncate().into_any_element()
}

pub fn data_row(
    id: impl Into<ElementId>,
    group: &'static str,
    selected: bool,
    last: bool,
    palette: &Palette,
) -> Stateful<gpui_kit::Div> {
    let hover = palette.hover.opacity(0.6);
    let fill = palette.hover;
    h_flex()
        .id(id)
        .group(group)
        .relative()
        .flex_none()
        .w_full()
        .h(px(ROW_HEIGHT))
        .items_center()
        .cursor_pointer()
        .when(!last, |this| this.border_b_1().border_color(palette.line))
        .role(gpui_kit::accesskit::Role::Row)
        .map(|this| {
            if selected {
                this.bg(fill)
            } else {
                this.hover(move |style| style.bg(hover))
            }
        })
}

pub fn head_cell(text: &'static str) -> Part {
    part(|base, _| base.h_flex().h_full().px(px(12.)).whitespace_nowrap()).child(text)
}

pub fn cell() -> Part {
    part(|base, _| base.h_flex().h_full().px(px(12.)).min_w_0().items_center())
}

pub fn cell_text(text: impl Into<SharedString>, palette: &Palette) -> AnyElement {
    div()
        .min_w_0()
        .text_size(px(13.))
        .line_height(px(18.))
        .text_color(palette.ink_muted)
        .font_features(crate::ui::tabular())
        .child(label(text.into()).ltr().truncate())
        .into_any_element()
}

pub struct FilterOption {
    pub title: &'static str,
    pub count: usize,
    pub action: Action,
}

pub fn filter_menu(id: &'static str, selected: usize, options: Vec<FilterOption>) -> AnyElement {
    let current = options
        .get(selected)
        .map(|option| option.title)
        .unwrap_or("All");
    let options = Rc::new(options);
    crate::ui::popup_menu::menu(id)
        .trigger(
            button(id, format!("Status: {current}"))
                .outline()
                .sm()
                .icon(IconName::ListFilter),
        )
        .anchor(gpui_kit::Anchor::TopRight)
        .width(220.)
        .content(move |mut content, _, _| {
            for (index, option) in options.iter().enumerate() {
                let action = option.action.clone();
                content = content.item(
                    crate::ui::popup_menu::MenuEntry::new(
                        format!("{id}-{index}"),
                        option.title,
                        move |window, cx| action(window, cx),
                    )
                    .check(index == selected)
                    .count(option.count),
                );
            }
            content
        })
        .into_any_element()
}
