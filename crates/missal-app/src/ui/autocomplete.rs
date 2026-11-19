use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::component::h_flex;
use gpui_kit::{
    AnchoredPositionMode, AnyElement, App, Bounds, Context, Entity, EntityId, Focusable,
    InteractiveElement, IntoElement, ParentElement, Pixels, ScrollHandle, Size, Styled,
    Subscription, WeakEntity, Window, canvas, div, point, px,
};
use missal_core::{SUGGESTION_LIMIT, Suggestion, SuggestionKey};
use missal_text::listbox::{
    self, Complete, Confirm, Dismiss, ListNav, Reveal, SelectNext, SelectPageDown, SelectPageUp,
    SelectPrevious, listbox, listbox_row, popover,
};
use missal_text::{
    Align, Attr, AttributedText, FontFamily, Label, LineHeight, TextInput, TextInputEvent,
    TextStyle, to_rgba,
};

use crate::state::Suggestions;
use crate::theme::Palette;
use crate::ui::menu::listbox_theme;

pub const SUGGEST: &str = "Suggest";
pub const SUGGEST_OPEN: &str = "SuggestOpen";
const ROW_HEIGHT: f32 = 36.0;
const PAGE: usize = 4;

pub fn bind_keys(cx: &mut App) {
    listbox::bind_keys(cx, "SuggestOpen > TextInput");
    listbox::bind_reveal_keys(cx, "Suggest > TextInput");
}

struct Binding {
    input: WeakEntity<TextInput>,
    key: SuggestionKey,
    _subscription: Subscription,
}

struct Open {
    input: EntityId,
    items: Vec<Suggestion>,
    nav: ListNav,
    reveal: bool,
}

pub struct Autocomplete {
    suggestions: Entity<Suggestions>,
    bindings: HashMap<EntityId, Binding>,
    open: Option<Open>,
    dismissed: Option<(EntityId, String)>,
    accepting: bool,
    sizes: Rc<RefCell<HashMap<EntityId, Size<Pixels>>>>,
    scroll: ScrollHandle,
    _observe: Subscription,
}

impl Autocomplete {
    pub fn new(suggestions: Entity<Suggestions>, cx: &mut Context<Self>) -> Autocomplete {
        let observe = cx.observe(&suggestions, |this, _, cx| this.refresh(cx));
        Autocomplete {
            suggestions,
            bindings: HashMap::new(),
            open: None,
            dismissed: None,
            accepting: false,
            sizes: Rc::default(),
            scroll: ScrollHandle::new(),
            _observe: observe,
        }
    }

    pub fn attach(
        &mut self,
        input: &Entity<TextInput>,
        key: SuggestionKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let subscription = cx.subscribe_in(
            input,
            window,
            |this, input, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Change => this.typed(input, window, cx),
                TextInputEvent::Blur => this.close_for(input.entity_id(), cx),
                TextInputEvent::Focus | TextInputEvent::Submit => {}
            },
        );
        self.bindings.insert(
            input.entity_id(),
            Binding {
                input: input.downgrade(),
                key,
                _subscription: subscription,
            },
        );
    }

    pub fn detach(&mut self, input: &Entity<TextInput>, cx: &mut Context<Self>) {
        let id = input.entity_id();
        self.bindings.remove(&id);
        self.sizes.borrow_mut().remove(&id);
        self.close_for(id, cx);
    }

    pub fn is_attached(&self, input: &Entity<TextInput>) -> bool {
        self.bindings.contains_key(&input.entity_id())
    }

    pub fn open_input(&self) -> Option<EntityId> {
        self.open.as_ref().map(|open| open.input)
    }

    pub fn items(&self) -> Vec<String> {
        self.open
            .iter()
            .flat_map(|open| open.items.iter().map(|item| item.value.clone()))
            .collect()
    }

    pub fn active(&self) -> Option<usize> {
        self.open.as_ref().and_then(|open| open.nav.active())
    }

    fn typed(&mut self, input: &Entity<TextInput>, window: &mut Window, cx: &mut Context<Self>) {
        if self.accepting || !input.focus_handle(cx).is_focused(window) {
            return;
        }
        let id = input.entity_id();
        let text = input.read(cx).text().to_owned();
        if let Some((dismissed, at)) = &self.dismissed
            && *dismissed == id
        {
            if *at == text {
                return;
            }
            self.dismissed = None;
        }
        if text.trim().is_empty() {
            self.close_for(id, cx);
            return;
        }
        self.show(id, &text, cx);
    }

    fn show(&mut self, id: EntityId, query: &str, cx: &mut Context<Self>) -> bool {
        let Some(binding) = self.bindings.get(&id) else {
            return false;
        };
        let items = self
            .suggestions
            .read(cx)
            .suggest(binding.key, query, SUGGESTION_LIMIT);
        if items.is_empty() {
            self.close_for(id, cx);
            return false;
        }
        let active = self
            .open
            .as_ref()
            .filter(|open| open.input == id)
            .and_then(|open| open.nav.active().map(|index| &open.items[index].value))
            .and_then(|value| items.iter().position(|item| &item.value == value));
        self.open = Some(Open {
            input: id,
            nav: ListNav::with_active(items.len(), active),
            items,
            reveal: true,
        });
        cx.notify();
        true
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.open_input() else {
            return;
        };
        let text = self
            .bindings
            .get(&id)
            .and_then(|binding| binding.input.upgrade())
            .map(|input| input.read(cx).text().to_owned());
        match text {
            Some(text) => {
                self.show(id, &text, cx);
            }
            None => self.close_for(id, cx),
        }
    }

    fn close_for(&mut self, id: EntityId, cx: &mut Context<Self>) {
        if self.open_input() == Some(id) {
            self.open = None;
            cx.notify();
        }
    }

    fn navigate(&mut self, f: impl FnOnce(&mut ListNav) -> bool, cx: &mut Context<Self>) {
        if let Some(open) = self.open.as_mut()
            && f(&mut open.nav)
        {
            open.reveal = true;
            cx.notify();
        }
    }

    fn hover(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(open) = self.open.as_mut()
            && open.nav.set_active(Some(index))
        {
            open.reveal = false;
            cx.notify();
        }
    }

    fn reveal(&mut self, id: EntityId, cx: &mut Context<Self>) -> bool {
        if self.open_input() == Some(id) {
            return true;
        }
        self.dismissed = None;
        let text = self
            .bindings
            .get(&id)
            .and_then(|binding| binding.input.upgrade())
            .map(|input| input.read(cx).text().to_owned())
            .unwrap_or_default();
        self.show(id, &text, cx)
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(open) = self.open.take() else {
            return false;
        };
        let text = self
            .bindings
            .get(&open.input)
            .and_then(|binding| binding.input.upgrade())
            .map(|input| input.read(cx).text().to_owned())
            .unwrap_or_default();
        self.dismissed = Some((open.input, text));
        cx.notify();
        true
    }

    pub fn accept(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        let Some(open) = self.open.take() else {
            return false;
        };
        let Some(item) = open.items.get(index) else {
            self.open = Some(open);
            return false;
        };
        let value = item.value.clone();
        let input = self
            .bindings
            .get(&open.input)
            .and_then(|binding| binding.input.upgrade());
        if let Some(input) = input {
            self.accepting = true;
            input.update(cx, |input, cx| input.replace_all(value.clone(), cx));
            self.accepting = false;
        }
        self.dismissed = Some((open.input, value));
        cx.notify();
        true
    }

    fn accept_active(&mut self, cx: &mut Context<Self>) -> bool {
        match self.active() {
            Some(index) => self.accept(index, cx),
            None => {
                if let Some(id) = self.open_input() {
                    self.close_for(id, cx);
                }
                false
            }
        }
    }
}

fn suggestion_label(palette: &Palette, suggestion: &Suggestion) -> Label {
    let style = TextStyle {
        family: FontFamily::Interface,
        size: 16.0,
        weight: 400,
        italic: false,
        underline: false,
        strikethrough: false,
        color: to_rgba(palette.ink),
        background: None,
    };
    let mut text = AttributedText::new(suggestion.value.as_str(), style);
    for range in &suggestion.highlights {
        text.push(range.clone(), Attr::Weight(500));
    }
    Label::rich(text)
        .text_size(px(16.))
        .text_color(palette.ink)
        .rtl()
        .align(Align::Right)
        .leading(LineHeight::Px(30.))
        .truncate()
}

fn hints(palette: &Palette) -> AnyElement {
    let hint = |keys: &'static [&'static str], text: &'static str| {
        h_flex()
            .items_center()
            .gap(px(4.))
            .children(keys.iter().map(|key| crate::ui::kbd(*key)))
            .child(text)
    };
    h_flex()
        .h(px(32.))
        .px(px(12.))
        .gap(px(12.))
        .items_center()
        .text_size(px(12.))
        .text_color(palette.ink_faint)
        .child(hint(&["↑", "↓"], "Choose"))
        .child(hint(&["↵"], "Fill in"))
        .child(div().flex_1())
        .child(hint(&["Esc"], "Close"))
        .into_any_element()
}

pub fn suggest_field(
    autocomplete: &Entity<Autocomplete>,
    input: &Entity<TextInput>,
    field: impl IntoElement,
    cx: &App,
) -> AnyElement {
    let id = input.entity_id();
    let state = autocomplete.read(cx);
    if !state.bindings.contains_key(&id) {
        return field.into_any_element();
    }
    let open = state.open.as_ref().filter(|open| open.input == id);
    let sizes = state.sizes.clone();
    let size = sizes.borrow().get(&id).copied();
    let popup = open.zip(size).map(|(open, size)| {
        let palette = *Palette::get(cx);
        let hover = autocomplete.downgrade();
        let pick = autocomplete.downgrade();
        let rows = open
            .items
            .iter()
            .map(|item| listbox_row(suggestion_label(&palette, item)))
            .collect();
        let list = listbox("suggestions", listbox_theme(&palette))
            .rows(rows)
            .active(open.nav.active())
            .row_height(ROW_HEIGHT)
            .width(size.width)
            .rtl(true)
            .track_scroll(&state.scroll, open.reveal)
            .footer(hints(&palette))
            .on_hover(move |index, _, cx| {
                hover.update(cx, |this, cx| this.hover(index, cx)).ok();
            })
            .on_pick(move |index, _, cx| {
                pick.update(cx, |this, cx| this.accept(index, cx)).ok();
            });
        popover(
            Bounds::new(point(px(0.), px(0.)), size),
            AnchoredPositionMode::Local,
            true,
            list,
        )
    });
    let entity = autocomplete.clone();
    let act = move |f: fn(&mut Autocomplete, &mut Context<Autocomplete>) -> bool| {
        let entity = entity.clone();
        move |window: &mut Window, cx: &mut App| {
            let _ = window;
            if !entity.update(cx, f) {
                cx.propagate();
            }
        }
    };
    let next = act(|this, cx| {
        this.navigate(|nav| nav.step(1), cx);
        true
    });
    let previous = act(|this, cx| {
        this.navigate(|nav| nav.step(-1), cx);
        true
    });
    let page_down = act(|this, cx| {
        this.navigate(|nav| nav.page(1, PAGE), cx);
        true
    });
    let page_up = act(|this, cx| {
        this.navigate(|nav| nav.page(-1, PAGE), cx);
        true
    });
    let confirm = act(Autocomplete::accept_active);
    let complete = {
        let entity = autocomplete.clone();
        move |_: &mut Window, cx: &mut App| {
            entity.update(cx, Autocomplete::accept_active);
            cx.propagate();
        }
    };
    let dismiss = act(Autocomplete::dismiss);
    let reveal = {
        let entity = autocomplete.clone();
        move |_: &mut Window, cx: &mut App| {
            if !entity.update(cx, |this, cx| this.reveal(id, cx)) {
                cx.propagate();
            }
        }
    };
    div()
        .relative()
        .w_full()
        .key_context(if open.is_some() {
            SUGGEST_OPEN
        } else {
            SUGGEST
        })
        .on_action(move |_: &SelectNext, window, cx| next(window, cx))
        .on_action(move |_: &SelectPrevious, window, cx| previous(window, cx))
        .on_action(move |_: &SelectPageDown, window, cx| page_down(window, cx))
        .on_action(move |_: &SelectPageUp, window, cx| page_up(window, cx))
        .on_action(move |_: &Confirm, window, cx| confirm(window, cx))
        .on_action(move |_: &Complete, window, cx| complete(window, cx))
        .on_action(move |_: &Dismiss, window, cx| dismiss(window, cx))
        .on_action(move |_: &Reveal, window, cx| reveal(window, cx))
        .child(field)
        .child(
            canvas(
                move |bounds, _, _| {
                    sizes.borrow_mut().insert(id, bounds.size);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
        .children(popup)
        .into_any_element()
}
