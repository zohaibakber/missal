use std::collections::HashSet;
use std::sync::Arc;

use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::{WindowExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString,
    Styled, Subscription, Window, div, px,
};
use missal_auth::{Account, AuthError, LicenceStatus, Role, SessionKeeper, UserChanges};
use missal_text::{TextInput, TextInputEvent, label};

use crate::auth::{AuthModel, password_problem};
use crate::state::{Load, toast_success};
use crate::theme::{Palette, RADIUS};
use crate::ui::dialogs::{
    dialog_description, dialog_footer, dialog_header, dialog_surface, dialog_title, optical_top,
};
use crate::ui::input::{Kind, field, secret_input};
use crate::ui::library::day_month_year;
use crate::ui::{
    Intent, SegmentItem, alert, button, field_description, field_error, field_group, field_label,
    form_field, ghost_sm, segmented, skeleton_bar,
};

const PASSWORD_WIDTH: f32 = 420.;
const USERS_WIDTH: f32 = 760.;
const LIST_HEIGHT: f32 = 420.;

fn caption(palette: &Palette, text: impl Into<SharedString>) -> AnyElement {
    div()
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(palette.ink_muted)
        .child(text.into())
        .into_any_element()
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

pub fn open_users(window: &mut Window, cx: &mut App) {
    if keeper(cx).is_none() {
        return;
    }
    let panel = cx.new(UsersPanel::new);
    window.open_dialog(cx, move |dialog: Dialog, window, cx| {
        let palette = *Palette::get(cx);
        dialog_surface(dialog, &palette, USERS_WIDTH)
            .margin_top(optical_top(window, LIST_HEIGHT + 180.))
            .child(panel.clone())
    });
}

struct UsersPanel {
    users: Vec<Account>,
    load: Load,
    busy: HashSet<String>,
    problem: Option<SharedString>,
}

fn licence_text(user: &Account) -> String {
    let date = || day_month_year(user.licence.until_ms).unwrap_or_default();
    match user.licence.status {
        _ if user.role.is_admin() => "Always licensed".to_owned(),
        LicenceStatus::Trial => format!("Trial until {}", date()),
        LicenceStatus::Licensed => format!("Licensed until {}", date()),
        LicenceStatus::TrialEnded => format!("Trial ended {}", date()),
        LicenceStatus::Expired => format!("Licence ended {}", date()),
    }
}

impl UsersPanel {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut panel = UsersPanel {
            users: Vec::new(),
            load: Load::Loading,
            busy: HashSet::new(),
            problem: None,
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

    fn apply(
        &mut self,
        id: String,
        job: impl FnOnce(&SessionKeeper, &str) -> Result<Account, AuthError> + Send + 'static,
        announce: bool,
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
            async move { job(&keeper, &id) }
        });
        cx.spawn_in(window, async move |this, cx| {
            let outcome = task.await;
            this.update_in(cx, |this, window, cx| {
                this.busy.remove(&id);
                match outcome {
                    Ok(account) => {
                        if announce {
                            toast_success(
                                window,
                                cx,
                                format!("{}: {}", account.email, licence_text(&account)),
                            );
                        }
                        if let Some(user) = this.users.iter_mut().find(|user| user.id == account.id)
                        {
                            *user = account;
                        }
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
        self.apply(
            id,
            move |keeper, id| keeper.update_user(id, &changes),
            false,
            window,
            cx,
        );
    }

    fn role_picker(
        &self,
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
        segmented(items, selected).into_any_element()
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
        let mut details = Vec::new();
        if !user.name.trim().is_empty() {
            details.push(user.email.clone());
        }
        details.push(licence_text(user));
        if user.disabled {
            details.push("Disabled".to_owned());
        }
        let grant_id = user.id.clone();
        let toggle_id = user.id.clone();
        let disabled_now = user.disabled;
        h_flex()
            .id(("user-row", index))
            .px(px(10.))
            .py(px(8.))
            .gap(px(12.))
            .items_center()
            .rounded(px(RADIUS))
            .hover(|style| style.bg(palette.hover))
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
                    .child(caption(palette, details.join(" · "))),
            )
            .child(self.role_picker(&user.id, index, user.role, busy || own, cx))
            .child(
                ghost_sm(("user-licence", index), "Add 1 year")
                    .disabled(busy || user.role.is_admin())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.apply(
                            grant_id.clone(),
                            |keeper, id| keeper.grant_licence(id, 1),
                            true,
                            window,
                            cx,
                        );
                    })),
            )
            .child(
                ghost_sm(
                    ("user-toggle", index),
                    if disabled_now { "Enable" } else { "Disable" },
                )
                .disabled(busy || own)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.change(
                        toggle_id.clone(),
                        UserChanges {
                            disabled: Some(!disabled_now),
                            ..UserChanges::default()
                        },
                        window,
                        cx,
                    );
                })),
            )
            .into_any_element()
    }

    fn list(&mut self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        match &self.load {
            Load::Loading => v_flex()
                .gap(px(10.))
                .pt(px(8.))
                .children((0..4).map(|_| skeleton_bar(USERS_WIDTH - 40., 40.)))
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
}

impl Render for UsersPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        v_flex()
            .gap(px(12.))
            .child(dialog_title("People"))
            .child(caption(
                &palette,
                "People create their own accounts. Members use templates, publishers can also publish to the library, and administrators also manage people. Add 1 year extends a licence from today or from its current end, whichever is later.",
            ))
            .children(
                self.problem
                    .clone()
                    .map(|problem| alert(Intent::Danger).child(problem)),
            )
            .child(div().h(px(LIST_HEIGHT)).child(self.list(&palette, cx)))
            .child(
                dialog_footer()
                    .mt(px(12.))
                    .child(div().flex_1())
                    .child(
                        button("users-close", "Close")
                            .primary()
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
            )
    }
}
