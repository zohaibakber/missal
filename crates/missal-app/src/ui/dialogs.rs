use std::rc::Rc;

use gpui_kit::component::dialog::{Dialog, DialogEntrance};
use gpui_kit::component::{WindowExt, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, FocusHandle, FontWeight, IntoElement, ParentElement, SharedString, Styled,
    Window, div, px,
};
use missal_text::{LineHeight, label};

use crate::theme::{DIALOG_WIDTH, Palette, RADIUS_XL, hex_alpha};
use crate::ui::{ButtonKind, button};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Neutral,
    Primary,
    Danger,
    Ghost,
}

pub type ChoiceAction = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone)]
pub struct Choice {
    pub id: &'static str,
    pub label: SharedString,
    pub tone: Tone,
    pub action: ChoiceAction,
}

impl Choice {
    pub fn new(
        id: &'static str,
        label: impl Into<SharedString>,
        tone: Tone,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Choice {
        Choice {
            id,
            label: label.into(),
            tone,
            action: Rc::new(action),
        }
    }
}

pub fn dialog_surface(dialog: Dialog, palette: &Palette, width: f32) -> Dialog {
    let ring = if palette.dark {
        hex_alpha(0xffffff, 0.08)
    } else {
        hex_alpha(0x14110e, 0.08)
    };
    dialog
        .w(px(width))
        .close_button(false)
        .entrance(DialogEntrance::Fade)
        .bg(palette.raised)
        .border_color(ring)
        .rounded(px(RADIUS_XL))
        .p(px(20.))
        .pb(px(16.))
        .font_family(missal_text::UI_FAMILY)
        .text_color(palette.ink)
}

pub fn dialog_title(text: impl Into<SharedString>) -> AnyElement {
    div()
        .text_size(px(16.))
        .line_height(px(24.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label(text.into()).leading(LineHeight::Px(24.)))
        .into_any_element()
}

pub fn dialog_body(palette: &Palette, text: impl Into<SharedString>) -> AnyElement {
    div()
        .mt(px(6.))
        .text_size(px(13.))
        .line_height(px(20.))
        .text_color(palette.ink_muted)
        .child(label(text.into()).leading(LineHeight::Px(20.)))
        .into_any_element()
}

fn choice_button(choice: &Choice, focus: Option<&FocusHandle>) -> AnyElement {
    let action = choice.action.clone();
    let kind = match choice.tone {
        Tone::Neutral => ButtonKind::Outline,
        Tone::Primary => ButtonKind::Primary,
        Tone::Danger => ButtonKind::Danger,
        Tone::Ghost => ButtonKind::Ghost,
    };
    let mut element = button(choice.id, choice.label.clone()).kind(kind);
    if let Some(focus) = focus {
        element = element.track_focus(focus);
    }
    element
        .on_click(move |_, window, cx| {
            window.close_dialog(cx);
            action(window, cx);
        })
        .into_any_element()
}

pub fn choice_row(choices: &[Choice], focus: Option<(&'static str, &FocusHandle)>) -> AnyElement {
    let focus_for = |choice: &Choice| {
        focus
            .filter(|(id, _)| *id == choice.id)
            .map(|(_, handle)| handle)
    };
    h_flex()
        .mt(px(18.))
        .pb(px(4.))
        .items_center()
        .gap(px(8.))
        .children(
            choices
                .iter()
                .filter(|choice| choice.tone == Tone::Ghost)
                .map(|choice| choice_button(choice, focus_for(choice))),
        )
        .child(div().flex_1())
        .children(
            choices
                .iter()
                .filter(|choice| choice.tone != Tone::Ghost)
                .map(|choice| choice_button(choice, focus_for(choice))),
        )
        .into_any_element()
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
    let initial = choices
        .iter()
        .find(|choice| choice.tone == Tone::Neutral)
        .or_else(|| choices.first())
        .map(|choice| choice.id);
    let focus = cx.focus_handle();
    let focus_for_dialog = focus.clone();
    window.open_dialog(cx, move |dialog: Dialog, _, cx| {
        let palette = *Palette::get(cx);
        dialog_surface(dialog, &palette, DIALOG_WIDTH).child(
            v_flex()
                .child(dialog_title(title.clone()))
                .child(dialog_body(&palette, description.clone()))
                .child(choice_row(
                    &choices,
                    initial.map(|id| (id, &focus_for_dialog)),
                )),
        )
    });
    window.defer(cx, move |window, cx| window.focus(&focus, cx));
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
        "Discard unsaved changes?",
        "Leaving this page throws away what you changed here.",
        vec![
            Choice::new("keep", "Keep editing", Tone::Neutral, |_, _| {}),
            Choice::new("discard", "Discard changes", Tone::Danger, on_discard),
        ],
    );
}

pub fn about(window: &mut Window, cx: &mut App, data_folder: Option<String>, can_check: bool) {
    let version = env!("CARGO_PKG_VERSION");
    window.open_dialog(cx, move |dialog: Dialog, _, cx| {
        let palette = *Palette::get(cx);
        let row = |name: &'static str, value: SharedString| {
            h_flex()
                .items_start()
                .gap(px(12.))
                .text_size(px(13.))
                .line_height(px(20.))
                .child(
                    div()
                        .w(px(96.))
                        .flex_none()
                        .text_color(palette.ink_muted)
                        .child(name),
                )
                .child(div().flex_1().min_w_0().child(label(value).ltr()))
        };
        let folder = data_folder.clone().unwrap_or_else(|| "Not set".to_owned());
        dialog_surface(dialog, &palette, DIALOG_WIDTH).child(
            v_flex()
                .gap(px(6.))
                .child(dialog_title("About Missal"))
                .child(div().h(px(6.)))
                .child(row("Version", version.into()))
                .child(row("Data folder", folder.into()))
                .child(
                    h_flex()
                        .mt(px(12.))
                        .pb(px(4.))
                        .gap(px(8.))
                        .child(div().flex_1())
                        .child(
                            button("about-check-updates", "Check for updates")
                                .outline()
                                .disabled(!can_check)
                                .on_click(|_, window, cx| {
                                    window.close_dialog(cx);
                                    crate::app::check_for_updates(window, cx);
                                }),
                        )
                        .child(
                            button("about-close", "Close")
                                .primary()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        ),
                ),
        )
    });
}
