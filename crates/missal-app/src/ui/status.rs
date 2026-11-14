use gpui_kit::prelude::*;
use gpui_kit::{
    BorderStyle, Corners, Edges, Hsla, IntoElement, PathBuilder, Pixels, canvas, point, px, quad,
    transparent_black,
};
use missal_core::FirStatus;

pub fn status_icon(status: FirStatus, color: Hsla, size: Pixels) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let diameter: f32 = bounds.size.width.into();
            let center = bounds.center();
            let stroke = px(1.5);
            let solid = status == FirStatus::Closed;
            window.paint_quad(quad(
                bounds,
                Corners::all(bounds.size.width / 2.0),
                if solid { color } else { transparent_black() },
                Edges::all(stroke),
                color,
                BorderStyle::Solid,
            ));
            let inner = px(diameter / 2.0 - 3.0);
            let top = point(center.x, center.y - inner);
            let wedge = |large_arc: bool, to| {
                let mut path = PathBuilder::fill();
                path.move_to(center);
                path.line_to(top);
                path.arc_to(point(inner, inner), px(0.0), large_arc, true, to);
                path.close();
                path.build().ok()
            };
            let path = match status {
                FirStatus::UnderInvestigation => wedge(false, point(center.x, center.y + inner)),
                FirStatus::ChallanSubmitted => wedge(true, point(center.x - inner, center.y)),
                FirStatus::Open | FirStatus::Closed => None,
            };
            if let Some(path) = path {
                window.paint_path(path, color);
            }
            if solid {
                let unit = diameter / 14.0;
                let mut check = PathBuilder::stroke(px(1.4));
                check.move_to(point(
                    bounds.origin.x + px(4.0 * unit),
                    bounds.origin.y + px(7.2 * unit),
                ));
                check.line_to(point(
                    bounds.origin.x + px(6.2 * unit),
                    bounds.origin.y + px(9.2 * unit),
                ));
                check.line_to(point(
                    bounds.origin.x + px(10.0 * unit),
                    bounds.origin.y + px(4.8 * unit),
                ));
                if let Ok(path) = check.build() {
                    window.paint_path(path, gpui_kit::white());
                }
            }
        },
    )
    .size(size)
    .flex_shrink_0()
}
