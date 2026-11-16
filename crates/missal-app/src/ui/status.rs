use gpui_kit::component::h_flex;
use gpui_kit::{App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};

use missal_core::FirStatus;

use crate::theme::Palette;
use crate::ui::part::Part;
use crate::ui::{UrRole, pill, ur_inline};

pub const DOT: f32 = 6.0;

#[derive(IntoElement)]
pub struct StatusDot {
    status: FirStatus,
}

pub fn status_dot(status: FirStatus) -> StatusDot {
    StatusDot { status }
}

impl RenderOnce for StatusDot {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .flex_none()
            .size(px(DOT))
            .rounded_full()
            .bg(Palette::get(cx).status(self.status))
    }
}

pub fn status_label(status: FirStatus) -> gpui_kit::Div {
    h_flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .child(status_dot(status))
        .child(ur_inline(status.label(), UrRole::Body))
}

pub fn status_pill(status: FirStatus) -> Part {
    pill().h(px(26.)).child(status_dot(status)).child(
        div()
            .mt(px(-2.))
            .font_weight(gpui_kit::FontWeight::NORMAL)
            .child(ur_inline(status.label(), UrRole::Label)),
    )
}

pub fn status_badge(status: FirStatus) -> Part {
    status_pill(status).flex_row_reverse()
}
