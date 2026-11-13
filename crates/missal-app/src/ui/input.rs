use gpui_kit::{App, AppContext, Context, Entity, Window};
use missal_text::{Align, Direction, TextInput};

pub const URDU_SIZE: f32 = 15.0;
pub const LATIN_SIZE: f32 = 14.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Urdu,
    Numeric,
    Latin,
    Auto,
}

pub fn text_input(
    window: &mut Window,
    cx: &mut App,
    kind: Kind,
    placeholder: &str,
    text: &str,
) -> Entity<TextInput> {
    cx.new(|cx| configure(TextInput::new(window, cx), kind, placeholder, text))
}

pub fn text_area(
    window: &mut Window,
    cx: &mut App,
    placeholder: &str,
    text: &str,
    rows: usize,
) -> Entity<TextInput> {
    cx.new(|cx| {
        configure(TextInput::new(window, cx).multiline(rows, rows + 4), Kind::Urdu, placeholder, text)
    })
}

fn configure(input: TextInput, kind: Kind, placeholder: &str, text: &str) -> TextInput {
    let input = match kind {
        Kind::Urdu => input.direction(Direction::Rtl).align(Align::Start).text_size(URDU_SIZE),
        Kind::Numeric => input.direction(Direction::Ltr).align(Align::Right).text_size(LATIN_SIZE),
        Kind::Latin => input.direction(Direction::Ltr).align(Align::Start).text_size(LATIN_SIZE),
        Kind::Auto => input.direction(Direction::Auto).align(Align::Start).text_size(URDU_SIZE),
    };
    input.placeholder(placeholder.to_owned()).default_text(text.to_owned())
}

pub fn set_text<T: 'static>(input: &Entity<TextInput>, text: &str, cx: &mut Context<T>) {
    input.update(cx, |input, cx| input.set_text(text, cx));
}
