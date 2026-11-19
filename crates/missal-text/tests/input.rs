use gpui_kit::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, Window, div, px,
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
