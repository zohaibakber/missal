use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::h_flex;
use gpui_kit::prelude::*;
use gpui_kit::{
    App, AppContext, Context, ElementId, Entity, Focusable, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, RenderOnce, Styled, Window, div, px,
};
use missal_text::{Align, Direction, LineHeight, TextInput};

use crate::theme::{ICON_SM, INPUT_LATIN, INPUT_URDU, Palette, RADIUS};
use crate::ui::{icon_button, keycap, sized_icon};

pub const URDU_SIZE: f32 = 16.0;
pub const LATIN_SIZE: f32 = 14.0;
pub const SEARCH_SIZE: f32 = 15.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Search,
    SearchRtl,
    Urdu,
    UrduAuto,
    LatinNumeric,
    Date,
    Latin,
}

impl Kind {
    pub const fn direction(self) -> Direction {
        match self {
            Kind::Search => Direction::AutoLtr,
            Kind::SearchRtl | Kind::Urdu => Direction::Rtl,
            Kind::UrduAuto => Direction::Auto,
            Kind::LatinNumeric | Kind::Date | Kind::Latin => Direction::Ltr,
        }
    }

    pub const fn align(self) -> Align {
        match self {
            Kind::Search => Align::Start,
            Kind::Latin => Align::Left,
            Kind::SearchRtl | Kind::Urdu | Kind::UrduAuto | Kind::LatinNumeric | Kind::Date => {
                Align::Right
            }
        }
    }

    pub const fn text_size(self) -> f32 {
        match self {
            Kind::Search => SEARCH_SIZE,
            Kind::SearchRtl | Kind::Urdu | Kind::UrduAuto => URDU_SIZE,
            Kind::LatinNumeric | Kind::Date | Kind::Latin => LATIN_SIZE,
        }
    }

    pub const fn holds_urdu(self) -> bool {
        matches!(
            self,
            Kind::Search | Kind::SearchRtl | Kind::Urdu | Kind::UrduAuto
        )
    }

    pub const fn height(self) -> f32 {
        if self.holds_urdu() {
            INPUT_URDU
        } else {
            INPUT_LATIN
        }
    }

    pub const fn rtl_chrome(self) -> bool {
        matches!(self, Kind::SearchRtl)
    }

    pub const fn placeholder_direction(self) -> Option<Direction> {
        match self {
            Kind::UrduAuto | Kind::Urdu | Kind::LatinNumeric | Kind::Date => Some(Direction::Auto),
            _ => None,
        }
    }
}

fn configure(input: TextInput, kind: Kind, placeholder: &str, text: &str, cx: &App) -> TextInput {
    let palette = Palette::get(cx);
    let mut input = input
        .direction(kind.direction())
        .align(kind.align())
        .text_size(kind.text_size())
        .placeholder_color(palette.ink_faint)
        .placeholder(placeholder.to_owned())
        .default_text(text.to_owned());
    if let Some(direction) = kind.placeholder_direction() {
        input = input.placeholder_direction(direction);
    }
    input
}

pub fn field_input(
    window: &mut Window,
    cx: &mut App,
    kind: Kind,
    placeholder: &str,
    text: &str,
) -> Entity<TextInput> {
    cx.new(|cx| {
        let input = TextInput::new(window, cx).bare();
        let line = if kind.holds_urdu() { 30. } else { 20. };
        configure(input, kind, placeholder, text, cx).line_height(LineHeight::Px(line))
    })
}

pub fn set_text<T: 'static>(input: &Entity<TextInput>, text: &str, cx: &mut Context<T>) {
    input.update(cx, |input, cx| input.set_text(text, cx));
}

type ClearHandler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hint {
    None,
    Slash,
    Escape,
}

#[derive(IntoElement)]
pub struct Field {
    id: ElementId,
    input: Entity<TextInput>,
    kind: Kind,
    width: Option<Pixels>,
    height: Option<f32>,
    error: bool,
    show_focus: bool,
    search_icon: bool,
    clearable: bool,
    hint: Hint,
    on_clear: Option<ClearHandler>,
    on_calendar: Option<ClearHandler>,
}

pub fn field(id: impl Into<ElementId>, input: &Entity<TextInput>, kind: Kind) -> Field {
    Field {
        id: id.into(),
        input: input.clone(),
        kind,
        width: None,
        height: None,
        error: false,
        show_focus: false,
        search_icon: false,
        clearable: false,
        hint: Hint::None,
        on_clear: None,
        on_calendar: None,
    }
}

pub fn search_field(id: impl Into<ElementId>, input: &Entity<TextInput>, kind: Kind) -> Field {
    let mut field = field(id, input, kind);
    field.search_icon = true;
    field.clearable = true;
    field
}

impl Field {
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = Some(width.into());
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn error(mut self, error: bool) -> Self {
        self.error = error;
        self
    }

    pub fn show_focus(mut self) -> Self {
        self.show_focus = true;
        self
    }

    pub fn clearable(mut self, clearable: bool) -> Self {
        self.clearable = clearable;
        self
    }

    pub fn hint(mut self, hint: Hint) -> Self {
        self.hint = hint;
        self
    }

    pub fn on_clear(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_clear = Some(Rc::new(handler));
        self
    }

    pub fn on_calendar(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_calendar = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Field {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let focus = self.input.focus_handle(cx);
        let focused = self.show_focus || focus.is_focused(window);
        let empty = self.input.read(cx).text().is_empty();
        let rtl = self.kind.rtl_chrome();
        let icon = self.search_icon.then(|| {
            sized_icon(
                gpui_kit::assets::IconName::Search,
                ICON_SM,
                palette.ink_faint,
            )
        });
        let input_entity = self.input.clone();
        let clear = (self.clearable && !empty).then(|| {
            let input = input_entity.clone();
            let on_clear = self.on_clear.clone();
            icon_button(
                ("field-clear", self.input.entity_id().as_u64() as usize),
                IconName::X,
                "Clear search",
            )
            .keys("Escape")
            .dense()
            .on_click(move |_, window, cx| {
                input.update(cx, |input, cx| {
                    input.set_text("", cx);
                    input.focus(window, cx);
                });
                if let Some(handler) = on_clear.as_ref() {
                    handler(window, cx);
                }
            })
        });
        let hint = match self.hint {
            Hint::Slash if empty && !focused => Some(keycap(&palette, "/")),
            Hint::Escape => Some(keycap(&palette, "Esc")),
            _ => None,
        };
        let calendar = self.on_calendar.clone().map(|handler| {
            icon_button(
                ("field-calendar", self.input.entity_id().as_u64() as usize),
                IconName::Calendar,
                "Pick a date",
            )
            .keys("Alt+Down")
            .dense()
            .on_click(move |_, window, cx| handler(window, cx))
        });
        let mut shadows = palette.shadow_sm();
        if focused && !self.error {
            shadows.extend(palette.input_focus_ring());
        }
        let border = if self.error {
            palette.danger_text
        } else if focused {
            palette.accent
        } else {
            palette.line_strong
        };
        let height = self.height.unwrap_or(self.kind.height());
        let focus_target = self.input.clone();
        let mut start: Vec<gpui_kit::AnyElement> = Vec::new();
        let mut end: Vec<gpui_kit::AnyElement> = Vec::new();
        if let Some(calendar) = calendar {
            start.push(calendar.into_any_element());
        }
        if let Some(icon) = icon {
            start.push(icon.into_any_element());
        }
        if let Some(clear) = clear {
            end.push(clear.into_any_element());
        }
        if let Some(hint) = hint {
            end.push(hint.into_any_element());
        }
        if rtl {
            std::mem::swap(&mut start, &mut end);
            start.reverse();
            end.reverse();
        }
        h_flex()
            .id(self.id)
            .flex_none()
            .when_some(self.width, |this, width| this.w(width))
            .when(self.width.is_none(), |this| this.w_full())
            .h(px(height))
            .items_center()
            .gap(px(8.))
            .px(px(10.))
            .rounded(px(RADIUS))
            .border_1()
            .border_color(border)
            .bg(palette.surface)
            .shadow(shadows)
            .cursor_text()
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                focus_target.update(cx, |input, cx| input.focus(window, cx));
            })
            .children(start)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .when(self.kind.holds_urdu(), |this| this.mt(px(-1.)))
                    .child(self.input.clone()),
            )
            .children(end)
    }
}
