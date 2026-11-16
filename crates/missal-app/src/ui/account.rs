use std::collections::HashSet;
use std::sync::Arc;

use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::{WindowExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString,
    Styled, Subscription, Window, div, px,
};
use missal_auth::{Account, AuthError, NewUser, Role, SessionKeeper, UserChanges};
use missal_text::{TextInput, TextInputEvent, label};

use crate::auth::{AuthModel, password_problem};
use crate::state::{Load, toast_success};
use crate::theme::{Palette, RADIUS};
use crate::ui::dialogs::{dialog_surface, dialog_title, optical_top};
use crate::ui::input::{Kind, field, field_input, secret_input};
use crate::ui::{SegmentItem, button, ghost_sm, primary_sm, segmented, skeleton_bar};

const PASSWORD_WIDTH: f32 = 420.;
const USERS_WIDTH: f32 = 760.;
const LIST_HEIGHT: f32 = 340.;

fn caption(palette: &Palette, text: impl Into<SharedString>) -> AnyElement {
    div()
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(palette.ink_muted)
        .child(text.into())
        .into_any_element()
}

fn problem_line(palette: &Palette, problem: Option<SharedString>) -> Option<AnyElement> {
    problem.map(|problem| {
        div()
            .text_size(px(13.))
            .line_height(px(20.))
            .text_color(palette.danger_text)
            .child(problem)
            .into_any_element()
    })
}

fn keeper(cx: &App) -> Option<Arc<SessionKeeper>> {
    AuthModel::get(cx)?.read(cx).keeper()
}

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
            cx.notify();
            return;
        }
        if let Some(problem) = password_problem(&new, &confirm) {
            self.problem = Some(problem.into());
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
        let palette = *Palette::get(cx);
        let width = px(PASSWORD_WIDTH - 40.);
        v_flex()
            .gap(px(10.))
            .child(dialog_title("Change password"))
            .child(caption(
                &palette,
                "At least 10 characters. Other computers signed in to this account are signed out.",
            ))
            .child(field("password-current", &self.current, Kind::Latin).width(width))
            .child(field("password-new", &self.new, Kind::Latin).width(width))
            .child(field("password-confirm", &self.confirm, Kind::Latin).width(width))
            .children(problem_line(&palette, self.problem.clone()))
            .child(
                h_flex()
                    .mt(px(8.))
                    .pb(px(4.))
                    .gap(px(8.))
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
                        .disabled(self.busy)
                        .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
    }
}

pub fn open_users(window: &mut Window, cx: &mut App) {
    if keeper(cx).is_none() {
        return;
    }
    let panel = cx.new(|cx| UsersPanel::new(window, cx));
    window.open_dialog(cx, move |dialog: Dialog, window, cx| {
        let palette = *Palette::get(cx);
        dialog_surface(dialog, &palette, USERS_WIDTH)
            .margin_top(optical_top(window, LIST_HEIGHT + 260.))
            .child(panel.clone())
    });
}

struct UsersPanel {
    users: Vec<Account>,
    load: Load,
    busy: HashSet<String>,
    email: Entity<TextInput>,
    name: Entity<TextInput>,
    password: Entity<TextInput>,
    role: Role,
    adding: bool,
    problem: Option<SharedString>,
    resetting: Option<String>,
    reset_password: Entity<TextInput>,
    _subscriptions: Vec<Subscription>,
}

impl UsersPanel {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let email = field_input(window, cx, Kind::Latin, "Email", "");
        let name = field_input(window, cx, Kind::Latin, "Name (optional)", "");
        let password = secret_input(window, cx, "Temporary password");
        let reset_password = secret_input(window, cx, "New temporary password");
        let watch = |input: &Entity<TextInput>,
                     reset: bool,
                     window: &mut Window,
                     cx: &mut Context<Self>| {
            cx.subscribe_in(
                input,
                window,
                move |this, _, event: &TextInputEvent, window, cx| match event {
                    TextInputEvent::Submit if reset => this.reset(window, cx),
                    TextInputEvent::Submit => this.add(window, cx),
                    TextInputEvent::Change => {
                        this.problem = None;
                        cx.notify();
                    }
                    _ => {}
                },
            )
        };
        let subscriptions = vec![
            watch(&email, false, window, cx),
            watch(&name, false, window, cx),
            watch(&password, false, window, cx),
            watch(&reset_password, true, window, cx),
        ];
        let mut panel = UsersPanel {
            users: Vec::new(),
            load: Load::Loading,
            busy: HashSet::new(),
            email,
            name,
            password,
            role: Role::Member,
            adding: false,
            problem: None,
            resetting: None,
            reset_password,
            _subscriptions: subscriptions,
        };
        panel.reload(cx);
        panel
    }

    fn failed(&mut self, error: &AuthError, cx: &mut Context<Self>) -> SharedString {
        if error.ends_session()
            && let Some(auth) = AuthModel::get(cx)
        {
            auth.update(cx, |auth, cx| auth.end_session(cx));
        }
        error.to_string().into()
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(keeper) = keeper(cx) else {
            return;
        };
        let task = cx.background_spawn(async move { keeper.users() });
        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            this.update(cx, |this, cx| {
                match outcome {
                    Ok(users) => {
                        this.users = users;
                        this.load = Load::Ready;
                    }
                    Err(error) => {
                        let message = this.failed(&error, cx);
                        this.load = Load::Failed(message.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn replace_user(&mut self, account: Account) {
        match self.users.iter_mut().find(|user| user.id == account.id) {
            Some(user) => *user = account,
            None => {
                self.users.push(account);
                self.users.sort_by(|a, b| a.email.cmp(&b.email));
            }
        }
    }

    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keeper) = keeper(cx) else {
            return;
        };
        if self.adding {
            return;
        }
        let user = NewUser {
            email: self.email.read(cx).text().trim().to_owned(),
            name: self.name.read(cx).text().trim().to_owned(),
            role: self.role,
            password: self.password.read(cx).text().to_owned(),
        };
        if !user.email.contains('@') {
            self.problem = Some("Enter the person's email address.".into());
            cx.notify();
            return;
        }
        if let Some(problem) = password_problem(&user.password, &user.password) {
            self.problem = Some(format!("Temporary password: {problem}").into());
            cx.notify();
            return;
        }
        self.adding = true;
        cx.notify();
        let task = cx.background_spawn(async move { keeper.create_user(&user) });
        cx.spawn_in(window, async move |this, cx| {
            let outcome = task.await;
            this.update_in(cx, |this, window, cx| {
                this.adding = false;
                match outcome {
                    Ok(account) => {
                        let message = format!(
                            "Added {}. Give them the temporary password; they choose their own when they first sign in.",
                            account.email
                        );
                        this.replace_user(account);
                        for input in [&this.email, &this.name, &this.password] {
                            input.update(cx, |input, cx| input.set_text("", cx));
                        }
                        this.role = Role::Member;
                        toast_success(window, cx, message);
                    }
                    Err(error) => this.problem = Some(this.failed(&error, cx)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn change(
        &mut self,
        id: String,
        changes: UserChanges,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(keeper) = keeper(cx) else {
            return;
        };
        if !self.busy.insert(id.clone()) {
            return;
        }
        self.problem = None;
        cx.notify();
        let task = cx.background_spawn({
            let id = id.clone();
            async move { keeper.update_user(&id, &changes) }
        });
        cx.spawn_in(window, async move |this, cx| {
            let outcome = task.await;
            this.update_in(cx, |this, _, cx| {
                this.busy.remove(&id);
                match outcome {
                    Ok(account) => this.replace_user(account),
                    Err(error) => this.problem = Some(this.failed(&error, cx)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(keeper), Some(id)) = (keeper(cx), self.resetting.clone()) else {
            return;
        };
        let password = self.reset_password.read(cx).text().to_owned();
        if let Some(problem) = password_problem(&password, &password) {
            self.problem = Some(format!("Temporary password: {problem}").into());
            cx.notify();
            return;
        }
        if !self.busy.insert(id.clone()) {
            return;
        }
        cx.notify();
        let task = cx.background_spawn({
            let id = id.clone();
            async move { keeper.reset_password(&id, &password) }
        });
        cx.spawn_in(window, async move |this, cx| {
            let outcome = task.await;
            this.update_in(cx, |this, window, cx| {
                this.busy.remove(&id);
                match outcome {
                    Ok(account) => {
                        let message = format!(
                            "{} must choose a new password at their next sign-in.",
                            account.email
                        );
                        this.replace_user(account);
                        this.resetting = None;
                        this.reset_password
                            .update(cx, |input, cx| input.set_text("", cx));
                        toast_success(window, cx, message);
                    }
                    Err(error) => this.problem = Some(this.failed(&error, cx)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn role_picker(
        &self,
        palette: &Palette,
        id: &str,
        index: usize,
        current: Role,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let items = Role::ALL
            .iter()
            .enumerate()
            .map(|(position, role)| {
                let role = *role;
                let id = id.to_owned();
                SegmentItem::new(
                    ("user-role", index * 3 + position),
                    role.label(),
                    cx.listener(move |this, _, window, cx| {
                        if disabled || role == current {
                            return;
                        }
                        this.change(
                            id.clone(),
                            UserChanges {
                                role: Some(role),
                                ..UserChanges::default()
                            },
                            window,
                            cx,
                        );
                    }),
                )
            })
            .collect();
        let selected = Role::ALL
            .iter()
            .position(|role| *role == current)
            .unwrap_or(0);
        segmented(palette, items, selected).into_any_element()
    }

    fn row(
        &self,
        index: usize,
        user: &Account,
        palette: &Palette,
        own: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let busy = self.busy.contains(&user.id);
        let status = if user.disabled {
            "Disabled"
        } else if user.must_change_password {
            "Has a temporary password"
        } else {
            ""
        };
        let resetting = self.resetting.as_deref() == Some(user.id.as_str());
        let id = user.id.clone();
        let toggle_id = user.id.clone();
        let disabled_now = user.disabled;
        v_flex()
            .id(("user-row", index))
            .px(px(10.))
            .py(px(8.))
            .gap(px(8.))
            .rounded(px(RADIUS))
            .hover(|style| style.bg(palette.hover))
            .child(
                h_flex()
                    .gap(px(12.))
                    .items_center()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .line_height(px(20.))
                                    .text_color(if user.disabled {
                                        palette.ink_faint
                                    } else {
                                        palette.ink
                                    })
                                    .child(label(user.display_name().to_owned()).ltr().truncate()),
                            )
                            .child(caption(
                                palette,
                                match (user.name.trim().is_empty(), status.is_empty()) {
                                    (true, true) => String::new(),
                                    (true, false) => status.to_owned(),
                                    (false, true) => user.email.clone(),
                                    (false, false) => format!("{} · {status}", user.email),
                                },
                            )),
                    )
                    .child(self.role_picker(palette, &user.id, index, user.role, busy || own, cx))
                    .child(
                        ghost_sm(("user-reset", index), "Reset password")
                            .disabled(busy || own)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.resetting = if this.resetting.as_deref() == Some(id.as_str()) {
                                    None
                                } else {
                                    Some(id.clone())
                                };
                                this.reset_password
                                    .update(cx, |input, cx| input.set_text("", cx));
                                if this.resetting.is_some() {
                                    this.reset_password
                                        .update(cx, |input, cx| input.focus(window, cx));
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        ghost_sm(
                            ("user-toggle", index),
                            if disabled_now { "Enable" } else { "Disable" },
                        )
                        .disabled(busy || own)
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.change(
                                    toggle_id.clone(),
                                    UserChanges {
                                        disabled: Some(!disabled_now),
                                        ..UserChanges::default()
                                    },
                                    window,
                                    cx,
                                );
                            },
                        )),
                    ),
            )
            .when(resetting, |this| {
                this.child(
                    h_flex()
                        .gap(px(8.))
                        .items_center()
                        .child(
                            field("user-reset-password", &self.reset_password, Kind::Latin)
                                .width(px(280.)),
                        )
                        .child(
                            primary_sm(("user-reset-save", index), "Set temporary password")
                                .disabled(busy)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.reset(window, cx)),
                                ),
                        ),
                )
            })
            .into_any_element()
    }

    fn list(&mut self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        match &self.load {
            Load::Loading => v_flex()
                .gap(px(10.))
                .pt(px(8.))
                .children((0..4).map(|_| skeleton_bar(palette, USERS_WIDTH - 40., 40.)))
                .into_any_element(),
            Load::Failed(message) => v_flex()
                .pt(px(24.))
                .gap(px(10.))
                .items_center()
                .child(caption(palette, message.clone()))
                .child(
                    button("users-retry", "Try again")
                        .outline()
                        .sm()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.load = Load::Loading;
                            this.reload(cx);
                        })),
                )
                .into_any_element(),
            Load::Ready => {
                let own = AuthModel::get(cx)
                    .and_then(|auth| auth.read(cx).account().map(|account| account.id.clone()));
                let users = self.users.clone();
                v_flex()
                    .id("users-list")
                    .overflow_y_scroll()
                    .h_full()
                    .gap(px(2.))
                    .children(users.iter().enumerate().map(|(index, user)| {
                        let own = own.as_deref() == Some(user.id.as_str());
                        self.row(index, user, palette, own, cx)
                    }))
                    .into_any_element()
            }
        }
    }

    fn add_form(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let selected = Role::ALL
            .iter()
            .position(|role| *role == self.role)
            .unwrap_or(0);
        let roles = Role::ALL
            .iter()
            .enumerate()
            .map(|(position, role)| {
                let role = *role;
                SegmentItem::new(
                    ("new-user-role", position),
                    role.label(),
                    cx.listener(move |this, _, _, cx| {
                        this.role = role;
                        cx.notify();
                    }),
                )
            })
            .collect();
        v_flex()
            .gap(px(8.))
            .p(px(12.))
            .rounded(px(RADIUS))
            .border_1()
            .border_color(palette.line)
            .child(caption(palette, "Add a person"))
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(field("new-user-email", &self.email, Kind::Latin).width(px(240.)))
                    .child(field("new-user-name", &self.name, Kind::Latin).width(px(200.)))
                    .child(field("new-user-password", &self.password, Kind::Latin).width(px(220.))),
            )
            .child(
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(segmented(palette, roles, selected))
                    .child(div().flex_1())
                    .child(
                        primary_sm("new-user-add", if self.adding { "Adding…" } else { "Add" })
                            .disabled(self.adding)
                            .on_click(cx.listener(|this, _, window, cx| this.add(window, cx))),
                    ),
            )
            .into_any_element()
    }
}

impl Render for UsersPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        v_flex()
            .gap(px(12.))
            .child(dialog_title("People"))
            .child(caption(
                &palette,
                "Members use templates. Publishers can also publish to the library. Administrators also manage people.",
            ))
            .child(self.add_form(&palette, cx))
            .children(problem_line(&palette, self.problem.clone()))
            .child(div().h(px(LIST_HEIGHT)).child(self.list(&palette, cx)))
            .child(
                h_flex()
                    .pb(px(4.))
                    .child(div().flex_1())
                    .child(
                        button("users-close", "Close")
                            .primary()
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
            )
    }
}
