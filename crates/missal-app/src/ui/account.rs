use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::{WindowExt, v_flex};
use gpui_kit::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled,
    Subscription, Window, div, px,
};
use missal_auth::AuthError;
use missal_text::{TextInput, TextInputEvent};

use crate::auth::{AuthModel, password_problem, password_too_short};
use crate::state::toast_success;
use crate::theme::Palette;
use crate::ui::dialogs::{
    dialog_description, dialog_footer, dialog_header, dialog_surface, dialog_title, optical_top,
};
use crate::ui::input::{Kind, field, secret_input};
use crate::ui::{button, field_description, field_error, field_group, field_label, form_field};
const PASSWORD_WIDTH: f32 = 420.;

pub fn open_change_password(window: &mut Window, cx: &mut App) {
    let Some(auth) = AuthModel::get(cx) else {
        return;
    };
    let panel = cx.new(|cx| ChangePassword::new(auth, window, cx));
    let focus = panel.clone();
    window.open_dialog(cx, move |dialog: Dialog, window, cx| {
        let palette = *Palette::get(cx);
        dialog_surface(dialog, &palette, PASSWORD_WIDTH)
            .margin_top(optical_top(window, 320.))
            .child(panel.clone())
    });
    window.defer(cx, move |window, cx| {
        let input = focus.read(cx).current.clone();
        input.update(cx, |input, cx| input.focus(window, cx));
    });
}

struct ChangePassword {
    auth: Entity<AuthModel>,
    current: Entity<TextInput>,
    new: Entity<TextInput>,
    confirm: Entity<TextInput>,
    problem: Option<SharedString>,
    busy: bool,
    _subscriptions: Vec<Subscription>,
}

impl ChangePassword {
    fn new(auth: Entity<AuthModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let current = secret_input(window, cx, "Current password");
        let new = secret_input(window, cx, "New password");
        let confirm = secret_input(window, cx, "Repeat the new password");
        let watch = |input: &Entity<TextInput>, window: &mut Window, cx: &mut Context<Self>| {
            cx.subscribe_in(
                input,
                window,
                |this, _, event: &TextInputEvent, window, cx| match event {
                    TextInputEvent::Submit => this.save(window, cx),
                    TextInputEvent::Change => {
                        this.problem = None;
                        cx.notify();
                    }
                    _ => {}
                },
            )
        };
        let subscriptions = vec![
            watch(&current, window, cx),
            watch(&new, window, cx),
            watch(&confirm, window, cx),
        ];
        ChangePassword {
            auth,
            current,
            new,
            confirm,
            problem: None,
            busy: false,
            _subscriptions: subscriptions,
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let current = self.current.read(cx).text().to_owned();
        let new = self.new.read(cx).text().to_owned();
        let confirm = self.confirm.read(cx).text().to_owned();
        if current.is_empty() {
            self.problem = Some("Enter your current password.".into());
            self.current.update(cx, |input, cx| input.focus(window, cx));
            cx.notify();
            return;
        }
        if let Some(problem) = password_problem(&new, &confirm) {
            self.problem = Some(problem.into());
            let field = if password_too_short(&new) {
                &self.new
            } else {
                &self.confirm
            };
            field.update(cx, |input, cx| input.focus(window, cx));
            cx.notify();
            return;
        }
        self.busy = true;
        cx.notify();
        let task = self
            .auth
            .update(cx, |auth, cx| auth.change_password(current, new, cx));
        cx.spawn_in(window, async move |this, cx| {
            let outcome = task.await;
            this.update_in(cx, |this, window, cx| {
                this.busy = false;
                match outcome {
                    Ok(()) => {
                        window.close_dialog(cx);
                        toast_success(window, cx, "Password changed");
                    }
                    Err(AuthError::SessionEnded) => window.close_dialog(cx),
                    Err(error) => {
                        this.problem = Some(error.to_string().into());
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }
}

impl Render for ChangePassword {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = px(PASSWORD_WIDTH - 40.);
        v_flex()
            .child(
                dialog_header()
                    .child(dialog_title("Change password"))
                    .child(dialog_description(
                        "Other computers signed in to this account are signed out.",
                    )),
            )
            .child(
                field_group()
                    .mt(px(20.))
                    .child(
                        form_field().child(field_label("Current password")).child(
                            field("password-current", &self.current, Kind::Latin).width(width),
                        ),
                    )
                    .child(
                        form_field()
                            .child(field_label("New password"))
                            .child(field("password-new", &self.new, Kind::Latin).width(width))
                            .child(field_description("At least 10 characters.")),
                    )
                    .child(
                        form_field()
                            .child(field_label("Repeat the new password"))
                            .child(
                                field("password-confirm", &self.confirm, Kind::Latin).width(width),
                            )
                            .children(self.problem.clone().map(field_error)),
                    ),
            )
            .child(
                dialog_footer()
                    .child(div().flex_1())
                    .child(
                        button("password-cancel", "Cancel")
                            .outline()
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        button(
                            "password-save",
                            if self.busy {
                                "Saving…"
                            } else {
                                "Change password"
                            },
                        )
                        .primary()
                        .busy(self.busy)
                        .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
    }
}
