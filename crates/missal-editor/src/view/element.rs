use gpui_kit::{
    App, Bounds, ContentMask, CursorStyle, DispatchPhase, Element, ElementId, ElementInputHandler,
    Entity, GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, ScrollEvent,
    Style, Window, fill, point, px, relative, size,
};
use missal_page::paint::{PagePaint, paint_page};
use missal_page::{Page, Rect};

use super::Editor;
use crate::model::selection::{covered_cells, effective_range};

pub struct PagesElement {
    editor: Entity<Editor>,
}

impl PagesElement {
    pub fn new(editor: Entity<Editor>) -> PagesElement {
        PagesElement { editor }
    }
}

impl IntoElement for PagesElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

pub struct Frame {
    hitbox: Hitbox,
}

struct Scene {
    pages: Vec<(Page, Bounds<Pixels>)>,
    selection: Vec<Bounds<Pixels>>,
    cells: Vec<Bounds<Pixels>>,
    marked: Vec<Bounds<Pixels>>,
    caret: Option<Bounds<Pixels>>,
    zoom: f32,
}

fn to_screen(origin: gpui_kit::Point<Pixels>, scroll: (f32, f32), rect: Rect) -> Bounds<Pixels> {
    Bounds::new(
        point(
            origin.x + px(rect.x - scroll.0),
            origin.y + px(rect.y - scroll.1),
        ),
        size(px(rect.width), px(rect.height)),
    )
}

fn scene(editor: &Editor, bounds: Bounds<Pixels>, caret_on: bool) -> Scene {
    let origin = bounds.origin;
    let scroll = (editor.scroll.x, editor.scroll.y);
    let height = f32::from(bounds.size.height);
    let zoom = editor.placement.zoom;
    let mut pages = Vec::new();
    let mut shown = Vec::new();
    for (index, rect) in editor.placement.pages.iter().enumerate() {
        if rect.bottom() < scroll.1 || rect.y > scroll.1 + height {
            continue;
        }
        if let Some(page) = editor.packet.page(index) {
            shown.push(index);
            pages.push((page.clone(), to_screen(origin, scroll, *rect)));
        }
    }
    let selection_state = editor.core.selection().clone();
    let mut selection = Vec::new();
    let mut cells = Vec::new();
    if !selection_state.is_collapsed() {
        let document = editor.core.document();
        let (s, e) = effective_range(document, &selection_state);
        let covered = covered_cells(document, &selection_state);
        let push = |page: usize, rect: Rect, out: &mut Vec<Bounds<Pixels>>| {
            if shown.contains(&page)
                && let Some(canvas) = editor.page_rect_to_canvas(page, rect)
            {
                out.push(to_screen(origin, scroll, canvas));
            }
        };
        if covered.is_empty() {
            for (page, rect) in editor.packet.selection(&s, &e) {
                push(page, rect, &mut selection);
            }
        } else {
            for placed in editor.packet.cells(0) {
                if covered.iter().any(|c| c.as_slice() == placed.path.steps()) {
                    push(placed.page, placed.rect, &mut cells);
                }
            }
            for (path, range) in crate::model::edit::hosts_between(document, &s, &e) {
                let inside = crate::model::tree::cell_of(document, &path)
                    .is_some_and(|c| covered.contains(&c));
                if inside || range.is_empty() {
                    continue;
                }
                let a = crate::model::selection::pos(path.clone(), range.start);
                let b = crate::model::selection::pos(path, range.end);
                for (page, rect) in editor.packet.selection(&a, &b) {
                    push(page, rect, &mut selection);
                }
            }
        }
    }
    let mut marked = Vec::new();
    if let Some(m) = editor.core.marked() {
        let a = crate::model::selection::pos(m.path.clone(), m.range.start);
        let b = crate::model::selection::pos(m.path.clone(), m.range.end);
        for (page, rect) in editor.packet.selection(&a, &b) {
            if let Some(canvas) = editor.page_rect_to_canvas(page, rect) {
                let screen = to_screen(origin, scroll, canvas);
                let thickness = (1.5 * zoom).max(1.0);
                marked.push(Bounds::new(
                    point(
                        screen.origin.x,
                        screen.origin.y + screen.size.height - px(thickness),
                    ),
                    size(screen.size.width, px(thickness)),
                ));
            }
        }
    }
    let caret = if caret_on {
        editor.caret_canvas_rect(&selection_state.head).map(|rect| {
            let width = (2.0 * zoom).clamp(1.0, 2.0);
            let screen = to_screen(origin, scroll, rect);
            Bounds::new(
                point(screen.origin.x - px(width / 2.0), screen.origin.y),
                size(px(width), screen.size.height),
            )
        })
    } else {
        None
    };
    Scene {
        pages,
        selection,
        cells,
        marked,
        caret,
        zoom,
    }
}

impl Element for PagesElement {
    type RequestLayoutState = ();
    type PrepaintState = Frame;

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
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
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
        self.editor
            .update(cx, |editor, _| editor.layout_frame(bounds));
        Frame {
            hitbox: window.insert_hitbox(bounds, HitboxBehavior::Normal),
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        frame: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.editor.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        window.set_cursor_style(CursorStyle::IBeam, &frame.hitbox);
        let caret_on = self.editor.read(cx).caret_shown(window);
        let scene = scene(self.editor.read(cx), bounds, caret_on);
        let theme = super::theme::EditorTheme::get(cx);
        let names = self.editor.read(cx).fields.mode == missal_core::DisplayMode::Labels
            || self.editor.read(cx).fields.values.is_none();
        let caret_color: Hsla = theme.ink;
        let options = PagePaint {
            zoom: scene.zoom,
            paper: None,
            show_fields: true,
            field_text: Some(if names { theme.accent } else { theme.ink }),
            field_background: Some(theme.field_wash),
            unresolved_underline: Some(theme.notice),
            field_radius: 3.0,
            empty_text: Some(theme.notice_text),
            empty_background: Some(theme.notice_wash),
            empty_underline: Some(theme.notice),
            ink: Some(theme.ink),
        };
        let selection_color = theme.selection;
        let paper_shadows = theme.paper_shadows();
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for (_, page_bounds) in &scene.pages {
                let radius = gpui_kit::Corners::all(px(2.));
                window.paint_drop_shadows(*page_bounds, radius, &paper_shadows);
                window.paint_quad(fill(*page_bounds, theme.paper).corner_radii(radius));
            }
            for rect in scene.cells.iter().chain(&scene.selection) {
                window.paint_quad(fill(*rect, selection_color));
            }
            for (page, page_bounds) in &scene.pages {
                paint_page(page, page_bounds.origin, &options, window, cx);
            }
            for underline in &scene.marked {
                window.paint_quad(fill(*underline, caret_color));
            }
            if let Some(caret) = scene.caret {
                window.paint_quad(fill(caret, caret_color));
            }
        });
        let editor = self.editor.clone();
        let hitbox = frame.hitbox.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble
                && event.button == MouseButton::Left
                && hitbox.is_hovered(window)
            {
                editor.update(cx, |editor, cx| editor.mouse_down(event, window, cx));
                cx.stop_propagation();
            }
        });
        let editor = self.editor.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble && editor.read(cx).is_dragging() {
                if event.pressed_button != Some(MouseButton::Left) {
                    editor.update(cx, |editor, _| editor.end_drag());
                    return;
                }
                editor.update(cx, |editor, cx| editor.drag_to(event.position, window, cx));
            }
        });
        let editor = self.editor.clone();
        window.on_mouse_event(move |_: &MouseUpEvent, phase, _window, cx| {
            if phase == DispatchPhase::Bubble && editor.read(cx).is_dragging() {
                editor.update(cx, |editor, _| editor.end_drag());
            }
        });
        let editor = self.editor.clone();
        let hitbox = frame.hitbox.clone();
        window.on_mouse_event(move |event: &ScrollEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                editor.update(cx, |editor, cx| editor.on_scroll(event, cx));
                cx.stop_propagation();
            }
        });
    }
}
