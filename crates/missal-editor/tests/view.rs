mod common;

use common::*;
use gpui_kit::test::{TestInput, TestWindowExt};
use gpui_kit::{
    AppContext, Context, ElementInputHandler, Entity, IntoElement, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, Window, div, point, px,
};
use missal_core::document::{Inline, TextFormat};
use missal_core::{Document, FieldMarkers};
use missal_editor::{Editor, EditorEvent, EditorFields};
use missal_page::BlockPath;

struct Host {
    editor: Entity<Editor>,
    events: Vec<EditorEvent>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(900.))
            .h(px(700.))
            .flex()
            .flex_col()
            .child(self.editor.clone())
    }
}

fn mod_key(key: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("cmd-{key}")
    } else {
        format!("ctrl-{key}")
    }
}

fn setup(
    cx: &mut TestAppContext,
    document: Document,
) -> (Entity<Host>, Entity<Editor>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        missal_text::init(cx);
        missal_editor::init(cx);
    });
    let (host, cx) = cx.add_window_view(|window, cx| {
        let editor = cx.new(|cx| Editor::new(window, cx));
        cx.subscribe(&editor, |host: &mut Host, _, event: &EditorEvent, _| {
            host.events.push(event.clone())
        })
        .detach();
        Host {
            editor,
            events: Vec::new(),
        }
    });
    let editor = host.read_with(cx, |h, _| h.editor.clone());
    cx.update(|window, cx| {
        editor.update(cx, |e, cx| {
            e.load(
                document,
                EditorFields::labels(catalog(), FieldMarkers::default()),
                false,
                cx,
            );
            e.focus(window, cx);
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.render_frame(cx));
    (host, editor, cx)
}

fn text_at(editor: &Entity<Editor>, cx: &mut VisualTestContext, steps: &[u32]) -> String {
    editor.read_with(cx, |e, _| {
        e.core().block_text(&BlockPath::new(steps)).unwrap()
    })
}

fn head(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> (Vec<u32>, usize) {
    editor.read_with(cx, |e, _| {
        let h = &e.selection().head;
        (h.path.steps().to_vec(), h.offset)
    })
}

fn screen_of(
    editor: &Entity<Editor>,
    cx: &mut VisualTestContext,
    steps: &[u32],
    offset: usize,
) -> gpui_kit::Point<gpui_kit::Pixels> {
    editor.read_with(cx, |e, _| {
        let b = e
            .position_screen_bounds(&missal_editor::model::selection::pos(
                BlockPath::new(steps),
                offset,
            ))
            .expect("painted");
        point(b.origin.x, b.origin.y + b.size.height / 2.0)
    })
}

#[gpui_kit::test]
fn typing_urdu_and_visual_arrows_in_rtl(cx: &mut TestAppContext) {
    let (host, editor, cx) = setup(cx, doc(vec![p("")]));
    cx.simulate_input("سلام دنیا");
    assert_eq!(text_at(&editor, cx, &[0]), "سلام دنیا");
    assert!(host.read_with(cx, |h, _| {
        h.events.contains(&EditorEvent::DirtyChanged(true))
    }));
    cx.simulate_keystrokes("right");
    assert_eq!(
        head(&editor, cx).1,
        "سلام دنی".len(),
        "right moves backwards in rtl"
    );
    cx.simulate_keystrokes("left");
    assert_eq!(head(&editor, cx).1, "سلام دنیا".len());
    cx.simulate_keystrokes("home");
    assert_eq!(head(&editor, cx).1, 0);
    cx.simulate_keystrokes("end");
    assert_eq!(head(&editor, cx).1, "سلام دنیا".len());
    cx.simulate_keystrokes("shift-right shift-right");
    let range = editor.read_with(cx, |e, _| e.selection().ordered());
    assert_eq!(range.0.offset, "سلام دن".len());
    cx.simulate_keystrokes("backspace");
    assert_eq!(text_at(&editor, cx, &[0]), "سلام دن");
    cx.simulate_keystrokes(&mod_key("z"));
    assert_eq!(text_at(&editor, cx, &[0]), "سلام دنیا");
}

#[gpui_kit::test]
fn enter_up_down_and_page_keys(cx: &mut TestAppContext) {
    let blocks: Vec<_> = (0..60)
        .map(|i| {
            p(&format!(
                "سطر نمبر {i} مدعی مقدمہ نے تھانہ صدر میں درخواست دی"
            ))
        })
        .collect();
    let (_, editor, cx) = setup(cx, doc(blocks));
    cx.simulate_keystrokes("down down");
    assert_eq!(head(&editor, cx).0, vec![2]);
    cx.simulate_keystrokes("up");
    assert_eq!(head(&editor, cx).0, vec![1]);
    let before = editor.read_with(cx, |e, _| e.scroll_offset().1);
    cx.simulate_keystrokes("pagedown pagedown");
    cx.update(|window, cx| window.render_frame(cx));
    let (path, _) = head(&editor, cx);
    assert!(path[0] > 5, "page down moves far: {path:?}");
    assert!(
        editor.read_with(cx, |e, _| e.scroll_offset().1) > before,
        "the view follows the caret"
    );
    cx.simulate_keystrokes(&mod_key("end"));
    cx.update(|window, cx| window.render_frame(cx));
    assert_eq!(head(&editor, cx).0, vec![59]);
    let caret = editor.read_with(cx, |e, _| {
        let head = e.selection().head.clone();
        (
            e.caret_canvas_rect(&head).unwrap(),
            e.scroll_offset().1,
            e.viewport_bounds().unwrap(),
        )
    });
    assert!(
        caret.0.y >= caret.1 && caret.0.bottom() <= caret.1 + f32::from(caret.2.size.height),
        "caret is scrolled into view"
    );
    cx.simulate_keystrokes("enter");
    cx.simulate_input("نیا");
    assert_eq!(text_at(&editor, cx, &[60]), "نیا");
}

#[gpui_kit::test]
fn ime_composition_marks_and_commits(cx: &mut TestAppContext) {
    let (_, editor, cx) = setup(cx, doc(vec![p("نام ")]));
    cx.simulate_keystrokes("end");
    let bounds = editor.read_with(cx, |e, _| e.viewport_bounds().unwrap());
    cx.update(|window, cx| {
        let mut ime = TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window);
        ime.compose_text(None, "ک", None, cx);
        assert_eq!(ime.marked_text_range(cx), Some(4..5));
        ime.compose_text(None, "کی", Some(2..2), cx);
        assert_eq!(ime.marked_text_range(cx), Some(4..6));
        assert_eq!(ime.selected_text_range(cx).map(|s| s.range), Some(6..6));
        ime.commit_text("کیا", cx);
        assert_eq!(ime.marked_text_range(cx), None);
    });
    assert_eq!(text_at(&editor, cx, &[0]), "نام کیا");
    let candidate = cx.update(|window, cx| {
        editor.update(cx, |e, cx| {
            use gpui_kit::EntityInputHandler;
            e.bounds_for_range(4..7, bounds, window, cx)
        })
    });
    let candidate = candidate.expect("candidate window bounds");
    assert!(
        bounds.contains(&candidate.origin),
        "candidate window is placed over the text"
    );
    cx.simulate_keystrokes(&mod_key("z"));
    assert_eq!(text_at(&editor, cx, &[0]), "نام ");
}

#[gpui_kit::test]
fn typing_markers_converts_to_a_field_chip(cx: &mut TestAppContext) {
    let (_, editor, cx) = setup(cx, doc(vec![p("")]));
    cx.simulate_input("جرم: «جرم» درج");
    let fields = editor.read_with(cx, |e, _| e.core().field_count());
    assert_eq!(fields, 1);
    let spans = editor.read_with(cx, |e, _| {
        e.packet()
            .text_block(0, &BlockPath::top(0))
            .unwrap()
            .fields()
            .len()
    });
    assert_eq!(
        spans, 1,
        "the page engine lays the field out as a field mark"
    );
    cx.simulate_keystrokes("home");
    let before = "جرم: ".len();
    let mut trail = Vec::new();
    for _ in 0..8 {
        cx.simulate_keystrokes("left");
        trail.push(head(&editor, cx).1);
    }
    assert!(
        trail
            .iter()
            .all(|o| *o <= before || *o >= before + missal_page::OBJECT_LEN),
        "never inside the field: {trail:?}"
    );
    assert!(
        trail.contains(&(before + missal_page::OBJECT_LEN)),
        "the field is crossed in one step: {trail:?}"
    );
}

#[gpui_kit::test]
fn mouse_click_drag_and_double_click(cx: &mut TestAppContext) {
    let (_, editor, cx) = setup(
        cx,
        doc(vec![
            p("پہلا پیراگراف یہاں"),
            p("دوسرا پیراگراف"),
            para(vec![text("فیلڈ "), field(offence()), text(" آخر")]),
        ]),
    );
    let start = screen_of(&editor, cx, &[0], 0);
    let end = screen_of(&editor, cx, &[1], "دوسرا".len());
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: start,
        modifiers: Modifiers::default(),
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(MouseMoveEvent {
        position: end,
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::default(),
    });
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: end,
        modifiers: Modifiers::default(),
        click_count: 1,
    });
    let (s, e) = editor.read_with(cx, |ed, _| ed.selection().ordered());
    assert_eq!(s.path, BlockPath::top(0));
    assert_eq!(e.path, BlockPath::top(1));
    assert_eq!(e.offset, "دوسرا".len());
    let shift = screen_of(&editor, cx, &[1], "دوسرا پیرا".len());
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: shift,
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::default()
        },
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: shift,
        modifiers: Modifiers::default(),
        click_count: 1,
    });
    let (_, e) = editor.read_with(cx, |ed, _| ed.selection().ordered());
    assert_eq!(e.offset, "دوسرا پیرا".len(), "shift-click extends");
    let word = screen_of(&editor, cx, &[0], 2);
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: word,
        modifiers: Modifiers::default(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: word,
        modifiers: Modifiers::default(),
        click_count: 2,
    });
    let (s, e) = editor.read_with(cx, |ed, _| ed.selection().ordered());
    assert_eq!(
        (s.offset, e.offset),
        (0, "پہلا".len()),
        "double-click selects the word"
    );
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: word,
        modifiers: Modifiers::default(),
        click_count: 3,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: word,
        modifiers: Modifiers::default(),
        click_count: 3,
    });
    let (s, e) = editor.read_with(cx, |ed, _| ed.selection().ordered());
    assert_eq!(
        (s.offset, e.offset),
        (0, "پہلا پیراگراف یہاں".len()),
        "triple-click selects the paragraph"
    );
    let target = editor.read_with(cx, |ed, _| {
        let at = |o| missal_editor::model::selection::pos(BlockPath::top(2), o);
        let rects = ed.packet().selection(
            &at("فیلڈ ".len()),
            &at("فیلڈ ".len() + missal_page::OBJECT_LEN),
        );
        let (page, rect) = rects[0];
        let b = ed.page_to_screen(page, rect).unwrap();
        point(
            b.origin.x + b.size.width / 2.0,
            b.origin.y + b.size.height / 2.0,
        )
    });
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: target,
        modifiers: Modifiers::default(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: target,
        modifiers: Modifiers::default(),
        click_count: 2,
    });
    assert_eq!(
        text_at(&editor, cx, &[2]),
        "فیلڈ «جرم» آخر",
        "double-click on a field reveals its name"
    );
    let (s, e) = editor.read_with(cx, |ed, _| ed.selection().ordered());
    assert_eq!((s.offset, e.offset), ("فیلڈ «".len(), "فیلڈ «جرم".len()));
}

#[gpui_kit::test]
fn clipboard_round_trips_fields_and_formatting(cx: &mut TestAppContext) {
    let (_, editor, cx) = setup(
        cx,
        doc(vec![
            para(vec![styled("موٹا ", "", 1), field(fir_no())]),
            p(""),
        ]),
    );
    cx.simulate_keystrokes(&format!("{} {}", mod_key("a"), mod_key("a")));
    cx.simulate_keystrokes(&mod_key("c"));
    let clipboard = cx.read_from_clipboard().and_then(|c| c.text());
    assert_eq!(
        clipboard.as_deref(),
        Some("موٹا مقدمہ نمبر\n"),
        "plain text carries the field labels"
    );
    cx.simulate_keystrokes(&mod_key("end"));
    cx.simulate_keystrokes(&mod_key("v"));
    let count = editor.read_with(cx, |e, _| e.core().field_count());
    assert_eq!(count, 2, "internal paste keeps fields");
    let bold = editor.read_with(cx, |e, _| {
        e.core()
            .inlines_at(&BlockPath::top(1))
            .unwrap()
            .iter()
            .any(|i| matches!(i, Inline::Text(t) if t.format().contains(TextFormat::BOLD)))
    });
    assert!(bold, "internal paste keeps formatting");
    cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
        "باہر سے\nدوسری سطر «جرم»".into(),
    ));
    cx.simulate_keystrokes(&mod_key("v"));
    let count = editor.read_with(cx, |e, _| e.core().field_count());
    assert_eq!(count, 3, "external text with a token converts");
}

#[gpui_kit::test]
fn toolbar_formatting_and_insert_field_popover(cx: &mut TestAppContext) {
    let (_, editor, cx) = setup(cx, doc(vec![p("متن")]));
    cx.simulate_keystrokes("end shift-home");
    cx.update(|window, cx| window.click("editor-bold", cx));
    let bold = editor.read_with(cx, |e, _| e.core().flag_state(TextFormat::BOLD));
    assert!(bold);
    cx.simulate_keystrokes("end");
    cx.update(|window, cx| window.click("insert-field-trigger", cx));
    cx.run_until_parked();
    assert!(editor.read_with(cx, |e, _| e.field_menu_is_open()));
    cx.simulate_input("بینک");
    cx.update(|window, cx| window.render_frame(cx));
    let choices = editor.read_with(cx, |e, _| e.field_choices());
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0].0, "Custom");
    cx.update(|window, cx| window.click(("field-choice", 0usize), cx));
    cx.run_until_parked();
    assert!(!editor.read_with(cx, |e, _| e.field_menu_is_open()));
    assert_eq!(editor.read_with(cx, |e, _| e.core().field_count()), 1);
    cx.simulate_input("ب");
    assert_eq!(
        text_at(&editor, cx, &[0]),
        format!("متن{}ب", missal_page::OBJECT),
        "focus returns to the document"
    );
}

#[gpui_kit::test]
fn values_and_labels_switch_keeps_caret_and_history(cx: &mut TestAppContext) {
    let (_, editor, cx) = setup(cx, doc(vec![para(vec![text("نمبر "), field(fir_no())])]));
    cx.simulate_keystrokes("end");
    cx.simulate_input("x");
    let before = editor.read_with(cx, |e, _| (e.selection().clone(), e.core().undo_depth()));
    cx.update(|_, cx| {
        editor.update(cx, |e, cx| {
            let fields = EditorFields {
                mode: missal_core::DisplayMode::Values,
                ..e.fields().clone()
            };
            e.set_fields(fields, cx);
        })
    });
    let after = editor.read_with(cx, |e, _| (e.selection().clone(), e.core().undo_depth()));
    assert_eq!(before, after);
}

#[gpui_kit::test]
fn tab_moves_between_table_cells(cx: &mut TestAppContext) {
    let (_, editor, cx) = setup(cx, doc(vec![grid(2, 2)]));
    cx.simulate_keystrokes("tab");
    assert_eq!(head(&editor, cx).0, vec![0, 0, 1, 0]);
    cx.simulate_input("ق");
    assert_eq!(text_at(&editor, cx, &[0, 0, 1, 0]), "قخانہ 01");
    cx.simulate_keystrokes("shift-tab");
    assert_eq!(head(&editor, cx).0, vec![0, 0, 0, 0]);
    cx.simulate_keystrokes("down");
    assert_eq!(
        head(&editor, cx).0,
        vec![0, 1, 0, 0],
        "down follows the column"
    );
}

#[gpui_kit::test]
fn zoom_keys_and_read_only(cx: &mut TestAppContext) {
    let (_, editor, cx) = setup(cx, doc(vec![p("متن")]));
    let fit = editor.read_with(cx, |e, _| e.zoom());
    assert!((0.25..=1.0).contains(&fit));
    cx.simulate_keystrokes(&mod_key("="));
    cx.update(|window, cx| window.render_frame(cx));
    assert!(editor.read_with(cx, |e, _| e.zoom()) > fit);
    cx.simulate_keystrokes(&mod_key("0"));
    cx.update(|window, cx| window.render_frame(cx));
    assert!((editor.read_with(cx, |e, _| e.zoom()) - fit).abs() < 1e-4);
    cx.update(|_, cx| editor.update(cx, |e, cx| e.set_read_only(true, cx)));
    cx.simulate_input("x");
    assert_eq!(text_at(&editor, cx, &[0]), "متن");
}
