use gpui_kit::component::h_flex;
use gpui_kit::{Div, ParentElement, Styled, div, px};
use missal_core::FirStatus;

use crate::theme::Palette;
use crate::ui::{UrRole, pill, ur_inline};

pub const DOT: f32 = 8.0;

pub fn status_dot(palette: &Palette, status: FirStatus) -> Div {
    div()
        .flex_none()
        .size(px(DOT))
        .rounded_full()
        .bg(palette.status(status))
}

pub fn status_label(palette: &Palette, status: FirStatus) -> Div {
    h_flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .child(status_dot(palette, status))
        .child(
            div()
                .text_color(palette.ink)
                .child(ur_inline(status.label(), UrRole::Body)),
        )
}

pub fn status_pill(palette: &Palette, status: FirStatus) -> Div {
    pill(palette).child(status_dot(palette, status)).child(
        div()
            .mt(px(-2.))
            .text_color(palette.ink)
            .child(ur_inline(status.label(), UrRole::Label)),
    )
}
