use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dialog::{Dialog, DialogFooter};
use gpui_kit::component::{StyledExt, WindowExt, v_flex};
use gpui_kit::{App, IntoElement, ParentElement, SharedString, Styled, Window, div, px};
use gpui_kit::prelude::*;
use missal_text::label;

use crate::theme::Palette;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Primary,
    Danger,
}

#[derive(Clone)]
pub struct Choice {
    pub id: &'static str,
    pub label: &'static str,
    pub tone: Tone,
    pub action: Rc<dyn Fn(&mut Window, &mut App)>,
}

impl Choice {
    pub fn new(
        id: &'static str,
        label: &'static str,
        tone: Tone,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Choice {
        Choice {
            id,
            label,
            tone,
            action: Rc::new(action),
        }
    }
}

pub fn open_choice(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    choices: Vec<Choice>,
) {
    let title: SharedString = title.into();
    let description: SharedString = description.into();
    window.open_dialog(cx, move |dialog: Dialog, _, cx| {
        let palette = *Palette::get(cx);
        let footer = DialogFooter::new().children(choices.iter().map(|choice| {
            let action = choice.action.clone();
            let button = Button::new(choice.id).label(choice.label);
            let button = match choice.tone {
                Tone::Neutral => button.outline(),
                Tone::Primary => button.primary(),
                Tone::Danger => button.danger(),
            };
            button.on_click(move |_, window, cx| {
                window.close_dialog(cx);
                action(window, cx);
            })
        }));
        dialog
            .w(px(440.))
            .close_button(false)
            .title(
                div()
                    .text_size(px(16.))
                    .font_semibold()
                    .child(label(title.clone())),
            )
            .child(
                v_flex().gap_2().pb_2().child(
                    div()
                        .text_size(px(14.))
                        .text_color(palette.muted_foreground)
                        .child(label(description.clone())),
                ),
            )
            .footer(footer)
    });
}

pub fn confirm_delete(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    confirm_label: &'static str,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
) {
    open_choice(
        window,
        cx,
        title,
        description,
        vec![
            Choice::new("cancel", "Cancel", Tone::Neutral, |_, _| {}),
            Choice::new("confirm", confirm_label, Tone::Danger, on_confirm),
        ],
    );
}

pub fn confirm_discard(
    window: &mut Window,
    cx: &mut App,
    on_discard: impl Fn(&mut Window, &mut App) + 'static,
) {
    open_choice(
        window,
        cx,
        "Unsaved changes",
        "Leaving this page will discard your unsaved changes.",
        vec![
            Choice::new("keep", "Keep editing", Tone::Neutral, |_, _| {}),
            Choice::new("discard", "Discard changes", Tone::Danger, on_discard),
        ],
    );
}
