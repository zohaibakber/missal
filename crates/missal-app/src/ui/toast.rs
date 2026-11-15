use std::rc::Rc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    AnimationExt, App, Context, Entity, FontWeight, Global, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, WeakEntity, Window,
    div, px,
};
use missal_text::{LineHeight, label};

use crate::state::ToastKind;
use crate::theme::{ICON_SM, Palette, RADIUS_LG, TOAST_MAX_WIDTH, motion_base, motion_exit};
use crate::ui::sized_icon;

pub const TOAST_DURATION: Duration = Duration::from_secs(4);
pub const TOAST_DURATION_WITH_ACTION: Duration = Duration::from_secs(8);
const HOVER_RECHECK: Duration = Duration::from_millis(500);

pub type ToastAction = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone)]
pub struct ToastSpec {
    pub kind: ToastKind,
    pub title: Option<SharedString>,
    pub message: SharedString,
    pub action: Option<(SharedString, ToastAction)>,
}

impl ToastSpec {
    pub fn new(kind: ToastKind, message: impl Into<SharedString>) -> ToastSpec {
        ToastSpec {
            kind,
            title: None,
            message: message.into(),
            action: None,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> ToastSpec {
        self.title = Some(title.into());
        self
    }

    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> ToastSpec {
        self.action = Some((label.into(), Rc::new(action)));
        self
    }
}

struct Live {
    id: usize,
    spec: ToastSpec,
    hovered: bool,
    leaving: bool,
}

pub struct ToastHost {
    toasts: Vec<Live>,
    next_id: usize,
}

pub struct ToastHostHandle(pub WeakEntity<ToastHost>);

impl Global for ToastHostHandle {}

impl ToastHost {
    pub fn new() -> ToastHost {
        ToastHost {
            toasts: Vec::new(),
            next_id: 0,
        }
    }

    pub fn get(cx: &App) -> Option<Entity<ToastHost>> {
        cx.try_global::<ToastHostHandle>()
            .and_then(|handle| handle.0.upgrade())
    }

    pub fn shown(&self) -> Vec<(ToastKind, Option<SharedString>, SharedString)> {
        self.toasts
            .iter()
            .filter(|toast| !toast.leaving)
            .map(|toast| {
                (
                    toast.spec.kind,
                    toast.spec.title.clone(),
                    toast.spec.message.clone(),
                )
            })
            .collect()
    }

    pub fn push(&mut self, spec: ToastSpec, cx: &mut Context<Self>) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let duration = if spec.action.is_some() {
            TOAST_DURATION_WITH_ACTION
        } else {
            TOAST_DURATION
        };
        self.toasts.push(Live {
            id,
            spec,
            hovered: false,
            leaving: false,
        });
        while self.toasts.iter().filter(|toast| !toast.leaving).count() > 4 {
            if let Some(oldest) = self
                .toasts
                .iter()
                .find(|toast| !toast.leaving)
                .map(|t| t.id)
            {
                self.dismiss(oldest, cx);
            }
        }
        cx.spawn(async move |this, cx| {
            let mut wait = duration;
            loop {
                cx.background_executor().timer(wait).await;
                let hovered = this
                    .update(cx, |this, _| {
                        this.toasts
                            .iter()
                            .find(|toast| toast.id == id)
                            .map(|toast| toast.hovered)
                    })
                    .ok()
                    .flatten();
                match hovered {
                    Some(true) => wait = HOVER_RECHECK,
                    Some(false) => {
                        this.update(cx, |this, cx| this.dismiss(id, cx)).ok();
                        break;
                    }
                    None => break,
                }
            }
        })
        .detach();
        cx.notify();
        id
    }

    pub fn dismiss(&mut self, id: usize, cx: &mut Context<Self>) {
        let Some(toast) = self.toasts.iter_mut().find(|toast| toast.id == id) else {
            return;
        };
        if toast.leaving {
            return;
        }
        if cx.reduce_motion() {
            self.toasts.retain(|toast| toast.id != id);
            cx.notify();
            return;
        }
        toast.leaving = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(crate::theme::MOTION_EXIT)
                .await;
            this.update(cx, |this, cx| {
                this.toasts.retain(|toast| toast.id != id);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.toasts.clear();
        cx.notify();
    }
}

impl Default for ToastHost {
    fn default() -> Self {
        ToastHost::new()
    }
}

pub fn toast_card(palette: &Palette, spec: &ToastSpec) -> gpui_kit::Div {
    let (icon, color) = match spec.kind {
        ToastKind::Success => (IconName::CircleCheck, palette.st_closed),
        ToastKind::Error => (IconName::CircleAlert, palette.danger_text),
        ToastKind::Warning => (IconName::TriangleAlert, palette.notice),
        ToastKind::Info => (IconName::Info, palette.ink_muted),
    };
    h_flex()
        .w_full()
        .items_center()
        .gap(px(10.))
        .py(px(10.))
        .px(px(12.))
        .rounded(px(RADIUS_LG))
        .bg(palette.raised)
        .shadow(palette.shadow_md())
        .text_size(px(13.))
        .line_height(px(20.))
        .text_color(palette.ink)
        .font_family(missal_text::UI_FAMILY)
        .child(sized_icon(icon, ICON_SM, color))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .children(spec.title.clone().map(|title| {
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .child(label(title).ltr().leading(LineHeight::Px(20.)))
                }))
                .child(
                    label(spec.message.clone())
                        .ltr()
                        .leading(LineHeight::Px(20.)),
                ),
        )
}

impl Render for ToastHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let accent = palette.accent;
        div()
            .absolute()
            .right(px(16.))
            .bottom(px(16.))
            .w(px(TOAST_MAX_WIDTH))
            .child(
                v_flex()
                    .w_full()
                    .gap(px(8.))
                    .children(self.toasts.iter().map(|toast| {
                        let id = toast.id;
                        let action = toast.spec.action.clone().map(|(text, handler)| {
                            div()
                                .id(("toast-action", id))
                                .flex_none()
                                .text_size(px(12.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(accent)
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    handler(window, cx);
                                    this.dismiss(id, cx);
                                }))
                                .child(text)
                        });
                        let card = div()
                            .id(("toast", id))
                            .occlude()
                            .on_hover(cx.listener(move |this, hovered: &bool, _, _| {
                                if let Some(toast) =
                                    this.toasts.iter_mut().find(|toast| toast.id == id)
                                {
                                    toast.hovered = *hovered;
                                }
                            }))
                            .child(toast_card(&palette, &toast.spec).children(action));
                        if toast.leaving {
                            card.with_animation(("toast-exit", id), motion_exit(), |this, delta| {
                                this.opacity(1. - delta)
                            })
                            .into_any_element()
                        } else {
                            card.with_animation(
                                ("toast-enter", id),
                                motion_base(),
                                |this, delta| this.opacity(delta).mt(px(8. * (1. - delta))),
                            )
                            .into_any_element()
                        }
                    })),
            )
    }
}
