use std::sync::Arc;

use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    App, Bounds, ContentMask, CursorStyle, DispatchPhase, Element, ElementId, ElementInputHandler,
    Entity, GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, LayoutId,
    MouseMoveEvent, MouseUpEvent, Pixels, Style, Window, point, px, relative, size,
};

use super::TextInput;
use crate::{
    layout::TextLayout,
    paint::{TextPaint, paint_text},
};

pub struct TextInputElement {
    input: Entity<TextInput>,
}

impl TextInputElement {
    pub fn new(input: Entity<TextInput>) -> Self {
        Self { input }
    }
}

pub struct InputPrepaint {
    layout: Arc<TextLayout>,
    placeholder: Option<Arc<TextLayout>>,
    hitbox: Hitbox,
    scroll: gpui_kit::Point<f32>,
}

impl IntoElement for TextInputElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextInputElement {
    type RequestLayoutState = ();
    type PrepaintState = InputPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let ((min_rows, max_rows), row, multiline) = self.input.update(cx, |input, cx| {
            (input.rows(), input.row_height(cx), input.is_multiline())
        });
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        if !multiline {
            style.size.height = px(row).into();
            return (window.request_layout(style, [], cx), ());
        }
        let input = self.input.clone();
        let layout_id = window.request_measured_layout(style, move |known, available, _, cx| {
            let width = known.width.map(f32::from).or(match available.width {
                gpui_kit::AvailableSpace::Definite(w) => Some(f32::from(w)),
                _ => None,
            });
            let height = input.update(cx, |input, cx| {
                let color = input.text_color(cx);
                let text = input.text().to_string();
                input.build_layout(&text, color, width, cx).height()
            });
            let height = height.clamp(row * min_rows as f32, row * max_rows as f32);
            size(px(width.unwrap_or(0.0)), px(height.ceil()))
        });
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let width: f32 = bounds.size.width.into();
        let height: f32 = bounds.size.height.into();
        let (layout, placeholder, scroll) = self.input.update(cx, |input, cx| {
            input.frame.bounds = Some(bounds);
            input.frame.wrap_width = Some(width);
            let layout = input.layout(cx);
            let caret = layout.caret(input.caret());
            let mut scroll = input.frame.scroll;
            if input.is_multiline() {
                scroll.x = 0.0;
                if layout.height() <= height {
                    scroll.y = 0.0;
                } else {
                    if caret.top + scroll.y < 0.0 {
                        scroll.y = -caret.top;
                    }
                    if caret.bottom + scroll.y > height {
                        scroll.y = height - caret.bottom;
                    }
                    scroll.y = scroll.y.clamp(height - layout.height(), 0.0);
                }
            } else {
                scroll.y = 0.0;
                let line = layout.lines().first().cloned();
                match line {
                    Some(line) if layout.natural_width() > width => {
                        if caret.x + scroll.x < 0.0 {
                            scroll.x = -caret.x;
                        }
                        if caret.x + scroll.x > width - 1.0 {
                            scroll.x = width - 1.0 - caret.x;
                        }
                        scroll.x = scroll.x.clamp(width - line.right, -line.left);
                    }
                    _ => scroll.x = 0.0,
                }
            }
            input.frame.scroll = scroll;
            input.frame.layout = Some(layout.clone());
            let placeholder = (input.text().is_empty() && !input.placeholder_text().is_empty())
                .then(|| input.build_placeholder_layout(Some(width), cx));
            (layout, placeholder, scroll)
        });
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        InputPrepaint {
            layout,
            placeholder,
            hitbox,
            scroll,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        window.set_cursor_style(CursorStyle::IBeam, &prepaint.hitbox);
        let input = self.input.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _window, cx| {
            if phase == DispatchPhase::Bubble && input.read(cx).is_dragging() {
                input.update(cx, |input, cx| input.on_drag(event.position, cx));
            }
        });
        let input = self.input.clone();
        window.on_mouse_event(move |_: &MouseUpEvent, phase, _window, cx| {
            if phase == DispatchPhase::Bubble && input.read(cx).is_dragging() {
                input.update(cx, |input, _| input.end_drag());
            }
        });

        let (anchor, head, marked) = self.input.read(cx).render_state();
        let caret_visible = self.input.read(cx).caret_visible(window);
        let multiline = self.input.read(cx).is_multiline();
        let theme = cx.theme();
        let selection_color = theme.selection;
        let caret_color = theme.caret;
        let row: f32 = prepaint
            .layout
            .lines()
            .first()
            .map(|l| l.bottom - l.top)
            .unwrap_or(f32::from(bounds.size.height));
        let mask = if multiline {
            Bounds::new(
                point(bounds.origin.x - px(10.), bounds.origin.y - px(4.)),
                size(bounds.size.width + px(20.), bounds.size.height + px(8.)),
            )
        } else {
            Bounds::new(
                point(bounds.origin.x - px(10.), bounds.origin.y - px(row)),
                size(
                    bounds.size.width + px(20.),
                    bounds.size.height + px(row * 2.0),
                ),
            )
        };
        let origin = point(
            bounds.origin.x + px(prepaint.scroll.x),
            bounds.origin.y + px(prepaint.scroll.y),
        );
        let layout = prepaint.layout.clone();
        let placeholder = prepaint.placeholder.clone();
        window.with_content_mask(Some(ContentMask { bounds: mask }), |window| {
            if let Some(placeholder) = &placeholder {
                paint_text(
                    placeholder,
                    bounds.origin,
                    &TextPaint::default(),
                    window,
                    cx,
                );
            }
            let selection = (anchor.index != head.index).then(|| {
                (
                    anchor.index.min(head.index)..anchor.index.max(head.index),
                    selection_color,
                )
            });
            let paint = TextPaint {
                color: None,
                selection,
                caret: caret_visible.then_some((head, caret_color)),
                marked,
            };
            paint_text(&layout, origin, &paint, window, cx);
        });
    }
}
