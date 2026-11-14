use gpui_kit::test::TestInput;
use gpui_kit::{
    AppContext, Context, ElementInputHandler, Entity, IntoElement, Modifiers, MouseButton,
    MouseDownEvent, MouseUpEvent, ParentElement, Render, Styled, TestAppContext, VisualTestContext,
    Window, div, point, px,
};
use missal_text::{TextInput, TextInputEvent};

struct Host {
    input: Entity<TextInput>,
    events: Vec<TextInputEvent>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(400.)).p(px(10.)).child(self.input.clone())
    }
}

fn setup(
    cx: &mut TestAppContext,
    build: impl FnOnce(TextInput) -> TextInput + 'static,
) -> (Entity<Host>, Entity<TextInput>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        missal_text::init(cx);
    });
    let (host, cx) = cx.add_window_view(|window, cx| {
        let input = cx.new(|cx| build(TextInput::new(window, cx)));
        cx.subscribe(&input, |host: &mut Host, _, event: &TextInputEvent, _| {
            host.events.push(event.clone());
        })
        .detach();
        Host {
            input,
            events: Vec::new(),
        }
    });
    let input = host.read_with(cx, |host, _| host.input.clone());
    cx.update(|window, cx| input.update(cx, |input, cx| input.focus(window, cx)));
    cx.run_until_parked();
    (host, input, cx)
}

fn text(input: &Entity<TextInput>, cx: &mut VisualTestContext) -> String {
    input.read_with(cx, |input, _| input.text().to_string())
}

fn selection(input: &Entity<TextInput>, cx: &mut VisualTestContext) -> std::ops::Range<usize> {
    input.read_with(cx, |input, _| input.selected_range())
}

fn caret(input: &Entity<TextInput>, cx: &mut VisualTestContext) -> usize {
    input.read_with(cx, |input, _| input.caret().index)
}

const SALAM: &str = "سلام دنیا";

#[gpui_kit::test]
fn typing_urdu_and_visual_arrows(cx: &mut TestAppContext) {
    let (host, input, cx) = setup(cx, |i| i);
    cx.simulate_input(SALAM);
    assert_eq!(text(&input, cx), SALAM);
    assert_eq!(caret(&input, cx), SALAM.len());
    let changes = host.read_with(cx, |h, _| {
        h.events
            .iter()
            .filter(|e| **e == TextInputEvent::Change)
            .count()
    });
    assert_eq!(changes, SALAM.chars().count());

    cx.simulate_keystrokes("left");
    assert_eq!(
        caret(&input, cx),
        SALAM.len(),
        "left at the logical end of RTL text is the left edge"
    );
    cx.simulate_keystrokes("right");
    let after_right = caret(&input, cx);
    assert_eq!(
        after_right,
        SALAM.len() - 'ا'.len_utf8(),
        "right moves back one letter in RTL"
    );
    cx.simulate_keystrokes("right right right right");
    assert_eq!(
        caret(&input, cx),
        "سلام".len(),
        "the zero-width space stop is skipped"
    );
    cx.simulate_keystrokes("home");
    assert_eq!(caret(&input, cx), 0);
    cx.simulate_keystrokes("left");
    assert_eq!(caret(&input, cx), 'س'.len_utf8());
    cx.simulate_keystrokes("end");
    assert_eq!(caret(&input, cx), SALAM.len());
}

#[gpui_kit::test]
fn selection_clipboard_and_undo(cx: &mut TestAppContext) {
    let (_, input, cx) = setup(cx, |i| i.default_text(SALAM));
    cx.simulate_keystrokes("home shift-left shift-left shift-left shift-left");
    assert_eq!(selection(&input, cx), 0.."سلام".len());
    let copy = if cfg!(target_os = "macos") {
        "cmd-c"
    } else {
        "ctrl-c"
    };
    let cut = if cfg!(target_os = "macos") {
        "cmd-x"
    } else {
        "ctrl-x"
    };
    let paste = if cfg!(target_os = "macos") {
        "cmd-v"
    } else {
        "ctrl-v"
    };
    let undo = if cfg!(target_os = "macos") {
        "cmd-z"
    } else {
        "ctrl-z"
    };
    let redo = if cfg!(target_os = "macos") {
        "cmd-shift-z"
    } else {
        "ctrl-y"
    };
    let all = if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    };
    cx.simulate_keystrokes(copy);
    assert_eq!(
        cx.read_from_clipboard().and_then(|c| c.text()).as_deref(),
        Some("سلام")
    );
    cx.simulate_keystrokes(cut);
    assert_eq!(text(&input, cx), " دنیا");
    cx.simulate_keystrokes("end");
    cx.simulate_keystrokes(paste);
    assert_eq!(text(&input, cx), " دنیاسلام");
    cx.simulate_keystrokes(undo);
    assert_eq!(text(&input, cx), " دنیا");
    cx.simulate_keystrokes(undo);
    assert_eq!(text(&input, cx), SALAM);
    cx.simulate_keystrokes(redo);
    assert_eq!(text(&input, cx), " دنیا");
    cx.simulate_keystrokes(all);
    assert_eq!(selection(&input, cx), 0.." دنیا".len());
    cx.simulate_keystrokes("backspace");
    assert_eq!(text(&input, cx), "");
}

#[gpui_kit::test]
fn typing_coalesces_into_one_undo_step(cx: &mut TestAppContext) {
    let (_, input, cx) = setup(cx, |i| i);
    cx.simulate_input("مقدمہ");
    cx.simulate_keystrokes("backspace");
    assert_eq!(text(&input, cx), "مقدم");
    let undo = if cfg!(target_os = "macos") {
        "cmd-z"
    } else {
        "ctrl-z"
    };
    cx.simulate_keystrokes(undo);
    assert_eq!(text(&input, cx), "مقدمہ");
    cx.simulate_keystrokes(undo);
    assert_eq!(text(&input, cx), "");
}

#[gpui_kit::test]
fn mixed_direction_arrows_follow_the_screen(cx: &mut TestAppContext) {
    let value = "نمبر 412 درج";
    let (_, input, cx) = setup(cx, move |i| i.default_text(value));
    cx.simulate_keystrokes("home");
    let x_of = |input: &Entity<TextInput>, cx: &mut VisualTestContext| {
        input.read_with(cx, |i, _| {
            let layout = i.frame_layout().expect("layout");
            (i.caret().index, layout.caret(i.caret()).x)
        })
    };
    let mut stops = vec![x_of(&input, cx)];
    for _ in 0..14 {
        cx.simulate_keystrokes("left");
        stops.push(x_of(&input, cx));
    }
    let moving: Vec<_> = stops
        .windows(2)
        .take_while(|w| w[0].0 != w[1].0 || w[0].1 != w[1].1)
        .collect();
    assert!(
        moving.iter().all(|w| w[1].1 < w[0].1 - 0.5),
        "every left press moves left: {stops:?}"
    );
    let digits = value.find("412").expect("digits");
    for d in digits..digits + 3 {
        assert!(
            stops.iter().any(|s| s.0 == d),
            "a stop inside the number at {d}: {stops:?}"
        );
    }
    assert_eq!(stops.last().expect("stops").0, value.len(), "{stops:?}");
}

#[gpui_kit::test]
fn word_and_delete_commands(cx: &mut TestAppContext) {
    let (_, input, cx) = setup(cx, |i| i.default_text("مقدمہ نمبر درج"));
    let word_delete = if cfg!(target_os = "macos") {
        "alt-backspace"
    } else {
        "ctrl-backspace"
    };
    cx.simulate_keystrokes(word_delete);
    assert_eq!(text(&input, cx), "مقدمہ نمبر ");
    cx.simulate_keystrokes("home delete");
    assert_eq!(text(&input, cx), "قدمہ نمبر ");
    let word_left = if cfg!(target_os = "macos") {
        "alt-left"
    } else {
        "ctrl-left"
    };
    cx.simulate_keystrokes(word_left);
    assert_eq!(
        caret(&input, cx),
        "قدمہ".len(),
        "word-left in RTL moves forward to the end of the word"
    );
}

#[gpui_kit::test]
fn enter_submits_single_line_and_breaks_multi_line(cx: &mut TestAppContext) {
    let (host, input, cx) = setup(cx, |i| i.default_text("ایک"));
    cx.simulate_keystrokes("enter");
    assert!(host.read_with(cx, |h, _| h.events.contains(&TextInputEvent::Submit)));
    assert_eq!(text(&input, cx), "ایک");
}

#[gpui_kit::test]
fn multi_line_enter_and_vertical_moves(cx: &mut TestAppContext) {
    let (_, area, cx) = setup(cx, |i| i.multiline(2, 6).default_text("ایک"));
    cx.simulate_keystrokes("enter");
    cx.simulate_input("دو");
    assert_eq!(text(&area, cx), "ایک\nدو");
    cx.simulate_keystrokes("up");
    assert!(caret(&area, cx) <= "ایک".len());
    cx.simulate_keystrokes("down");
    assert!(caret(&area, cx) > "ایک".len());
}

#[gpui_kit::test]
fn disabled_and_read_only(cx: &mut TestAppContext) {
    let (_, input, cx) = setup(cx, |i| i.default_text("ثابت").read_only(true));
    cx.simulate_input("x");
    cx.simulate_keystrokes("backspace");
    assert_eq!(text(&input, cx), "ثابت");
    let all = if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    };
    cx.simulate_keystrokes(all);
    assert_eq!(selection(&input, cx), 0.."ثابت".len());
}

#[gpui_kit::test]
fn disabled_ignores_input(cx: &mut TestAppContext) {
    let (_, disabled, cx) = setup(cx, |i| i.default_text("بند").disabled(true));
    cx.simulate_input("x");
    assert_eq!(text(&disabled, cx), "بند");
}

#[gpui_kit::test]
fn ime_composition(cx: &mut TestAppContext) {
    let (host, input, cx) = setup(cx, |i| i.default_text("نام "));
    let bounds = input
        .read_with(cx, |i, _| i.frame_bounds())
        .expect("painted");
    cx.update(|window, cx| {
        let mut ime = TestInput::new(ElementInputHandler::new(bounds, input.clone()), window);
        ime.compose_text(None, "ک", None, cx);
        assert_eq!(ime.marked_text_range(cx), Some(4..5));
        ime.compose_text(None, "کی", Some(2..2), cx);
        assert_eq!(ime.marked_text_range(cx), Some(4..6));
        assert_eq!(ime.selected_text_range(cx).map(|s| s.range), Some(6..6));
        ime.commit_text("کیا", cx);
        assert_eq!(ime.marked_text_range(cx), None);
        ime.compose_text(None, "´", None, cx);
        ime.compose_text(None, "", None, cx);
        assert_eq!(ime.marked_text_range(cx), None);
    });
    assert_eq!(text(&input, cx), "نام کیا");
    let undo = if cfg!(target_os = "macos") {
        "cmd-z"
    } else {
        "ctrl-z"
    };
    cx.simulate_keystrokes(undo);
    assert_eq!(text(&input, cx), "نام ");
    assert!(host.read_with(cx, |h, _| {
        h.events
            .iter()
            .filter(|e| **e == TextInputEvent::Change)
            .count()
            >= 3
    }));
}

#[gpui_kit::test]
fn mouse_click_double_click_and_drag(cx: &mut TestAppContext) {
    let value = "مقدمہ نمبر درج";
    let (_, input, cx) = setup(cx, move |i| i.default_text(value));
    let bounds = input
        .read_with(cx, |i, _| i.frame_bounds())
        .expect("painted");
    let right = point(bounds.right() - px(1.), bounds.center().y);
    let left = point(bounds.left() + px(1.), bounds.center().y);
    let click = |cx: &mut VisualTestContext, position, count, shift: bool| {
        let modifiers = Modifiers {
            shift,
            ..Modifiers::default()
        };
        cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers,
            click_count: count,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers,
            click_count: count,
        });
    };
    click(cx, right, 1, false);
    assert_eq!(
        caret(&input, cx),
        0,
        "clicking the right edge of RTL text puts the caret at the start"
    );
    let layout_left = input
        .read_with(cx, |i, _| i.frame_layout())
        .expect("layout");
    let line = layout_left.lines()[0].clone();
    let end_x = bounds.left() + px(line.left - 2.0);
    click(cx, point(end_x.max(left.x), left.y), 1, true);
    assert_eq!(selection(&input, cx), 0..value.len());
    let word = layout_left.caret(missal_text::TextPosition::downstream(
        value.find("نمبر").expect("word"),
    ));
    click(
        cx,
        point(bounds.left() + px(word.x - 6.0), left.y),
        2,
        false,
    );
    let start = value.find("نمبر").expect("word");
    assert_eq!(selection(&input, cx), start..start + "نمبر".len());
    click(cx, right, 3, false);
    assert_eq!(selection(&input, cx), 0..value.len());
}

#[gpui_kit::test]
fn boxed_input_is_inset_by_its_padding(cx: &mut TestAppContext) {
    let (_, input, cx) = setup(cx, |i| i.text_size(16.));
    let bounds = input.read_with(cx, |input, _| input.frame_bounds().unwrap());
    assert_eq!(bounds.origin.x, px(21.));
}

#[gpui_kit::test]
fn bare_input_draws_no_box_and_takes_the_line_height(cx: &mut TestAppContext) {
    let (_, input, cx) = setup(cx, |i| {
        i.bare()
            .text_size(16.)
            .line_height(missal_text::LineHeight::Px(30.))
    });
    let bounds = input.read_with(cx, |input, _| input.frame_bounds().unwrap());
    assert!(input.read_with(cx, |input, _| input.is_bare()));
    assert_eq!(bounds.origin.x, px(10.));
    assert_eq!(bounds.size.width, px(380.));
    assert_eq!(bounds.size.height, px(30.));
}

#[gpui_kit::test]
fn an_english_placeholder_decides_an_empty_auto_field(cx: &mut TestAppContext) {
    use missal_text::Direction;
    let (_, input, cx) = setup(cx, |i| i.direction(Direction::Auto).placeholder("Empty"));
    assert_eq!(
        input.read_with(cx, |input, _| input.resolved_direction()),
        Direction::Ltr
    );
}

#[gpui_kit::test]
fn placeholder_direction_does_not_flip_the_field(cx: &mut TestAppContext) {
    use missal_text::Direction;
    let (_, input, cx) = setup(cx, |i| {
        i.direction(Direction::Auto)
            .placeholder("Empty")
            .placeholder_direction(Direction::Ltr)
    });
    let read = |input: &Entity<TextInput>, cx: &mut VisualTestContext| {
        input.read_with(cx, |input, _| {
            (
                input.resolved_direction(),
                input.placeholder_resolved_direction(),
            )
        })
    };
    assert_eq!(read(&input, cx), (Direction::Rtl, Direction::Ltr));
    cx.simulate_input("بینک");
    assert_eq!(read(&input, cx).0, Direction::Rtl);
    cx.update(|_, cx| input.update(cx, |input, cx| input.set_text("Bank", cx)));
    assert_eq!(read(&input, cx).0, Direction::Ltr);
}

#[test]
fn direction_resolution_rules() {
    use missal_text::Direction::*;
    use missal_text::input::{resolve_direction, resolve_placeholder_direction};
    assert_eq!(resolve_direction(AutoLtr, "", "تلاش", None), Ltr);
    assert_eq!(resolve_direction(AutoLtr, "ڈکیتی", "", None), Rtl);
    assert_eq!(resolve_direction(AutoLtr, "11/20", "", None), Ltr);
    assert_eq!(resolve_direction(Auto, "", "", None), Rtl);
    assert_eq!(resolve_direction(Auto, "", "Search", None), Ltr);
    assert_eq!(resolve_direction(Auto, "", "Search", Some(Ltr)), Rtl);
    assert_eq!(resolve_direction(Rtl, "abc", "", None), Rtl);
    assert_eq!(resolve_placeholder_direction(Rtl, "Empty", Some(Ltr)), Ltr);
    assert_eq!(resolve_placeholder_direction(Rtl, "Empty", None), Rtl);
    assert_eq!(resolve_placeholder_direction(Rtl, "تلاش", Some(Auto)), Rtl);
}
