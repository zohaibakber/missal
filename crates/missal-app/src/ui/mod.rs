pub mod account;
pub mod admin;
pub mod autocomplete;
pub mod cheatsheet;
pub mod controls;
pub mod dialogs;
pub mod input;
pub mod library;
pub mod menu;
pub mod nav_menu;
pub mod palette;
pub mod part;
pub mod parts;
pub mod popup_menu;
pub mod print_preview;
pub mod sidebar;
pub mod status;
pub mod titlebar;
pub mod toast;
pub mod user_menu;
pub mod window_controls;

pub use controls::*;
pub use parts::*;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use gpui_kit::assets::IconName;
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::{Icon, h_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AnyView, App, BoxShadow, ClickEvent, Context, ElementId, FocusHandle, FontFeatures,
    FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, Render, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use missal_text::{Align, Label, LineHeight, label};

use crate::theme::{
    BUTTON_MD, BUTTON_SM, BUTTON_XS, ICON, ICON_BUTTON, ICON_BUTTON_DENSE, ICON_SM, Palette,
    RADIUS, RADIUS_LG, SPINNER_MIN_VISIBLE, SPINNER_SHOW_DELAY, TOOLTIP_DELAY, TOOLTIP_SKIP_DELAY,
    hex_alpha,
};

pub type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

pub fn tabular() -> FontFeatures {
    static FEATURES: LazyLock<FontFeatures> =
        LazyLock::new(|| FontFeatures(Arc::new(vec![("tnum".into(), 1)])));
    FEATURES.clone()
}

pub fn icon(name: IconName) -> Icon {
    Icon::new(name)
}

pub fn sized_icon(name: IconName, size: f32, color: Hsla) -> Icon {
    Icon::new(name).size(px(size)).text_color(color)
}

pub fn small_icon(name: IconName, color: Hsla) -> Icon {
    sized_icon(name, ICON, color)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UrRole {
    Label,
    Body,
    InputLabel,
    Title,
    Display,
}

impl UrRole {
    pub const fn size(self) -> f32 {
        match self {
            UrRole::Label => 14.0,
            UrRole::Body => 16.0,
            UrRole::InputLabel => 15.0,
            UrRole::Title => 18.0,
            UrRole::Display => 22.0,
        }
    }

    pub const fn line_height(self) -> f32 {
        match self {
            UrRole::Label => 26.0,
            UrRole::Body => 30.0,
            UrRole::InputLabel => 28.0,
            UrRole::Title => 34.0,
            UrRole::Display => 40.0,
        }
    }
}

pub fn ur_label(text: impl Into<SharedString>, role: UrRole) -> Label {
    label(text)
        .rtl()
        .leading(LineHeight::Px(role.line_height()))
        .text_size(px(role.size()))
}

pub fn ur_inline(text: impl Into<SharedString>, role: UrRole) -> Label {
    label(text)
        .rtl()
        .align(Align::Left)
        .leading(LineHeight::Px(role.line_height()))
        .text_size(px(role.size()))
}

pub fn isolate_urdu(text: &str) -> String {
    format!("\u{2067}{text}\u{2069}")
}

pub fn pressed_fill(palette: &Palette) -> Hsla {
    palette.line_input.opacity(0.64)
}

pub struct TooltipView {
    label: SharedString,
    keys: Option<SharedString>,
}

impl Render for TooltipView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        div().p(px(6.)).child(
            h_flex()
                .items_center()
                .gap(px(8.))
                .py(px(4.))
                .px(px(8.))
                .rounded(px(RADIUS))
                .bg(palette.raised)
                .shadow(palette.shadow_md())
                .text_color(palette.ink)
                .font_family(missal_text::UI_FAMILY)
                .text_size(px(12.))
                .line_height(px(16.))
                .child(self.label.clone())
                .children(self.keys.as_ref().map(|keys| kbd_group(keys.clone()))),
        )
    }
}

#[derive(Default)]
struct TooltipWarmth {
    open: usize,
    closed_at: Option<Instant>,
}

thread_local! {
    static TOOLTIPS: RefCell<TooltipWarmth> = RefCell::default();
}

pub fn tooltip_delay() -> Duration {
    TOOLTIPS.with_borrow(|warmth| {
        let warm = warmth.open > 0
            || warmth
                .closed_at
                .is_some_and(|at| at.elapsed() < TOOLTIP_SKIP_DELAY);
        if warm { Duration::ZERO } else { TOOLTIP_DELAY }
    })
}

pub fn tooltip(
    label: impl Into<SharedString>,
    keys: Option<&str>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let label: SharedString = label.into();
    let keys: Option<SharedString> = keys.map(|keys| SharedString::from(keys.to_owned()));
    move |_, cx| {
        let view = cx.new(|_| TooltipView {
            label: label.clone(),
            keys: keys.clone(),
        });
        TOOLTIPS.with_borrow_mut(|warmth| warmth.open += 1);
        cx.observe_release(&view, |_, cx| {
            TOOLTIPS.with_borrow_mut(|warmth| {
                warmth.open = warmth.open.saturating_sub(1);
                warmth.closed_at = Some(Instant::now());
            });
            cx.spawn(async move |cx| {
                cx.background_executor().timer(TOOLTIP_SKIP_DELAY).await;
                cx.update(|cx| cx.refresh_windows());
            })
            .detach();
        })
        .detach();
        view.into()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Outline,
    Ghost,
    GhostMuted,
    Danger,
    DangerOutline,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonSize {
    Xs,
    Sm,
    Md,
}

impl ButtonSize {
    const fn height(self) -> f32 {
        match self {
            ButtonSize::Xs => BUTTON_XS,
            ButtonSize::Sm => BUTTON_SM,
            ButtonSize::Md => BUTTON_MD,
        }
    }

    const fn padding(self) -> f32 {
        match self {
            ButtonSize::Xs => 7.,
            ButtonSize::Sm => 9.,
            ButtonSize::Md => 11.,
        }
    }

    const fn gap(self) -> f32 {
        match self {
            ButtonSize::Xs => 4.,
            ButtonSize::Sm => 6.,
            ButtonSize::Md => 8.,
        }
    }

    const fn radius(self) -> f32 {
        match self {
            ButtonSize::Xs => RADIUS,
            ButtonSize::Sm | ButtonSize::Md => RADIUS_LG,
        }
    }

    const fn text(self) -> (f32, f32) {
        match self {
            ButtonSize::Xs => (12., 16.),
            ButtonSize::Sm | ButtonSize::Md => (14., 20.),
        }
    }
}

#[derive(Clone)]
struct ButtonLook {
    fill: Option<Hsla>,
    hover: Option<Hsla>,
    pressed: Option<Hsla>,
    border: Hsla,
    hover_border: Option<Hsla>,
    text: Hsla,
    shadows: Vec<BoxShadow>,
}

fn button_look(palette: &Palette, kind: ButtonKind) -> ButtonLook {
    let transparent = gpui_kit::transparent_black();
    let control = palette.control_fill();
    let control_hover = palette.control_fill_hover();
    let outline_shadows = {
        let mut shadows = vec![palette.edge()];
        shadows.extend(palette.elevation_xs());
        shadows
    };
    let solid = |color: Hsla, text: Hsla| ButtonLook {
        fill: Some(color),
        hover: Some(color.opacity(0.9)),
        pressed: Some(color.opacity(0.9)),
        border: color,
        hover_border: Some(color.opacity(0.9)),
        text,
        shadows: vec![
            inset_shadow(hex_alpha(0xffffff, 0.16), 1.),
            BoxShadow {
                color: color.opacity(0.24),
                offset: gpui_kit::point(px(0.), px(1.)),
                blur_radius: px(2.),
                spread_radius: px(0.),
                inset: false,
            },
        ],
    };
    match kind {
        ButtonKind::Primary => solid(palette.action, palette.on_action),
        ButtonKind::Danger => solid(palette.danger, palette.on_danger),
        ButtonKind::Secondary => ButtonLook {
            fill: Some(palette.hover),
            hover: Some(palette.hover.opacity(0.9)),
            pressed: Some(palette.hover.opacity(0.8)),
            border: transparent,
            hover_border: None,
            text: palette.ink,
            shadows: Vec::new(),
        },
        ButtonKind::Outline => ButtonLook {
            fill: Some(control),
            hover: Some(control_hover),
            pressed: Some(control_hover),
            border: palette.line_input,
            hover_border: None,
            text: palette.ink,
            shadows: outline_shadows,
        },
        ButtonKind::DangerOutline => ButtonLook {
            fill: Some(control),
            hover: Some(control.blend(palette.danger.opacity(0.04))),
            pressed: Some(control.blend(palette.danger.opacity(0.04))),
            border: palette.line_input,
            hover_border: Some(palette.danger.opacity(0.32)),
            text: palette.danger_text,
            shadows: outline_shadows,
        },
        ButtonKind::Ghost | ButtonKind::GhostMuted => ButtonLook {
            fill: None,
            hover: Some(palette.hover),
            pressed: Some(palette.hover),
            border: transparent,
            hover_border: None,
            text: if kind == ButtonKind::Ghost {
                palette.ink
            } else {
                palette.ink_muted
            },
            shadows: Vec::new(),
        },
    }
}

pub(crate) fn inset_shadow(color: Hsla, y: f32) -> BoxShadow {
    BoxShadow {
        color,
        offset: gpui_kit::point(px(0.), px(y)),
        blur_radius: px(0.),
        spread_radius: px(0.),
        inset: true,
    }
}

fn with_focus_ring(palette: &Palette, mut shadows: Vec<BoxShadow>) -> Vec<BoxShadow> {
    shadows.extend(palette.focus_ring());
    shadows
}

#[derive(Default)]
struct BusyGate {
    busy: bool,
    visible: bool,
    shown_at: Option<Instant>,
    timer: Option<gpui_kit::Task<()>>,
}

fn busy_timer(
    gate: &Rc<RefCell<BusyGate>>,
    wait: Duration,
    window: &Window,
    cx: &App,
    apply: fn(&mut BusyGate),
) -> gpui_kit::Task<()> {
    let gate = gate.clone();
    window.spawn(cx, async move |cx| {
        cx.background_executor().timer(wait).await;
        apply(&mut gate.borrow_mut());
        cx.update(|window, _| window.refresh()).ok();
    })
}

fn busy_spinner_shown(id: &ElementId, busy: bool, window: &mut Window, cx: &mut App) -> bool {
    window.with_global_id(id.clone(), |global_id, window| {
        window.with_element_state(
            global_id,
            |gate: Option<Option<Rc<RefCell<BusyGate>>>>, window| {
                let gate = gate.flatten().or_else(|| busy.then(Default::default));
                let Some(gate) = gate else {
                    return (false, None);
                };
                let shown = {
                    let mut state = gate.borrow_mut();
                    if state.busy != busy {
                        state.busy = busy;
                        state.timer = None;
                        if busy && !state.visible {
                            state.timer =
                                Some(busy_timer(&gate, SPINNER_SHOW_DELAY, window, cx, |state| {
                                    if state.busy {
                                        state.visible = true;
                                        state.shown_at = Some(Instant::now());
                                    }
                                }));
                        } else if !busy && state.visible {
                            let shown_for = state
                                .shown_at
                                .map_or(SPINNER_MIN_VISIBLE, |at| at.elapsed());
                            let left = SPINNER_MIN_VISIBLE.saturating_sub(shown_for);
                            if left.is_zero() {
                                state.visible = false;
                            } else {
                                state.timer = Some(busy_timer(&gate, left, window, cx, |state| {
                                    if !state.busy {
                                        state.visible = false;
                                    }
                                }));
                            }
                        }
                    }
                    state.visible
                };
                let keep = {
                    let state = gate.borrow();
                    state.busy || state.visible
                };
                (shown, keep.then_some(gate))
            },
        )
    })
}

#[derive(IntoElement)]
pub struct ActionButton {
    id: ElementId,
    label: SharedString,
    icon: Option<IconName>,
    keys: Option<SharedString>,
    kind: ButtonKind,
    size: ButtonSize,
    disabled: bool,
    busy: bool,
    show_hover: bool,
    show_focus: bool,
    focus: Option<FocusHandle>,
    tab_index: isize,
    tooltip: Option<(SharedString, Option<SharedString>)>,
    on_click: Option<ClickHandler>,
}

pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> ActionButton {
    ActionButton {
        id: id.into(),
        label: label.into(),
        icon: None,
        keys: None,
        kind: ButtonKind::Outline,
        size: ButtonSize::Md,
        disabled: false,
        busy: false,
        show_hover: false,
        show_focus: false,
        focus: None,
        tab_index: 0,
        tooltip: None,
        on_click: None,
    }
}

pub fn primary_sm(id: impl Into<ElementId>, label: impl Into<SharedString>) -> ActionButton {
    button(id, label).kind(ButtonKind::Primary).sm()
}

pub fn ghost_sm(id: impl Into<ElementId>, label: impl Into<SharedString>) -> ActionButton {
    button(id, label).kind(ButtonKind::Ghost).sm()
}

pub fn danger_outline(id: impl Into<ElementId>, label: impl Into<SharedString>) -> ActionButton {
    button(id, label).kind(ButtonKind::DangerOutline).sm()
}

impl ActionButton {
    pub fn kind(mut self, kind: ButtonKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn primary(self) -> Self {
        self.kind(ButtonKind::Primary)
    }

    pub fn outline(self) -> Self {
        self.kind(ButtonKind::Outline)
    }

    pub fn secondary(self) -> Self {
        self.kind(ButtonKind::Secondary)
    }

    pub fn ghost(self) -> Self {
        self.kind(ButtonKind::Ghost)
    }

    pub fn danger(self) -> Self {
        self.kind(ButtonKind::Danger)
    }

    pub fn sm(mut self) -> Self {
        self.size = ButtonSize::Sm;
        self
    }

    pub fn xs(mut self) -> Self {
        self.size = ButtonSize::Xs;
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn keys(mut self, keys: impl Into<SharedString>) -> Self {
        self.keys = Some(keys.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }

    pub fn tab_index(mut self, tab_index: isize) -> Self {
        self.tab_index = tab_index;
        self
    }

    pub fn tooltip(mut self, label: impl Into<SharedString>, keys: Option<&str>) -> Self {
        self.tooltip = Some((
            label.into(),
            keys.map(|keys| SharedString::from(keys.to_owned())),
        ));
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    pub fn show_hover(mut self) -> Self {
        self.show_hover = true;
        self
    }

    pub fn show_focus(mut self) -> Self {
        self.show_focus = true;
        self
    }
}

impl RenderOnce for ActionButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let look = button_look(&palette, self.kind);
        let spinning = busy_spinner_shown(&self.id, self.busy, window, cx);
        let blocked = self.disabled || self.busy || spinning;
        let disabled = self.disabled || spinning;
        let (text_size, line) = self.size.text();
        let focus_shadows = with_focus_ring(&palette, look.shadows.clone());
        let resting = if self.show_focus {
            focus_shadows.clone()
        } else {
            look.shadows.clone()
        };
        let tooltip_spec = self.tooltip.clone().or_else(|| {
            self.keys
                .clone()
                .map(|keys| (self.label.clone(), Some(keys)))
        });
        let on_click = self.on_click.clone();
        let fill = if self.show_hover {
            look.hover.or(look.fill)
        } else {
            look.fill
        };
        let hover = look.hover;
        let pressed = look.pressed;
        let hover_border = look.hover_border;
        BaseButton::new(self.id)
            .disabled(disabled)
            .tab_index(self.tab_index)
            .when_some(self.focus.as_ref(), |this, focus| this.track_focus(focus))
            .accessibility_label(self.label.clone())
            .flex_none()
            .h(px(self.size.height()))
            .px(px(self.size.padding()))
            .gap(px(self.size.gap()))
            .rounded(px(self.size.radius()))
            .border_1()
            .border_color(look.border)
            .when_some(fill, |this, fill| this.bg(fill))
            .text_color(look.text)
            .text_size(px(text_size))
            .line_height(px(line))
            .font_weight(FontWeight::MEDIUM)
            .whitespace_nowrap()
            .cursor_pointer()
            .shadow(resting)
            .focus_visible(move |style| style.shadow(focus_shadows))
            .styles(|styles| styles.disabled(|style| style.opacity(0.64).shadow(Vec::new())))
            .when(!blocked, |this| {
                this.hover(move |style| {
                    let style = match hover {
                        Some(fill) => style.bg(fill),
                        None => style,
                    };
                    match hover_border {
                        Some(border) => style.border_color(border),
                        None => style,
                    }
                })
                .active(move |style| match pressed {
                    Some(fill) => style.bg(fill),
                    None => style,
                })
            })
            .when_some(tooltip_spec, |this, (label, keys)| {
                this.tooltip(tooltip(label, keys.as_deref()))
                    .tooltip_show_delay(tooltip_delay())
            })
            .when_some(on_click.filter(|_| !blocked), |this, handler| {
                this.on_click(move |event, window, cx| handler(event, window, cx))
            })
            .when(spinning, |this| {
                this.child(
                    div()
                        .mx(px(-2.))
                        .child(spinner(ICON, look.text.opacity(0.8))),
                )
            })
            .when_some(self.icon.filter(|_| !spinning), |this, name| {
                this.child(
                    div()
                        .mx(px(-2.))
                        .child(sized_icon(name, ICON, look.text.opacity(0.8))),
                )
            })
            .child(self.label)
    }
}

#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: IconName,
    label: SharedString,
    keys: Option<SharedString>,
    dense: bool,
    toggled: bool,
    disabled: bool,
    show_hover: bool,
    show_focus: bool,
    color: Option<Hsla>,
    focus: Option<FocusHandle>,
    on_click: Option<ClickHandler>,
    content: Option<AnyElement>,
}

pub fn icon_button(
    id: impl Into<ElementId>,
    icon: IconName,
    label: impl Into<SharedString>,
) -> IconButton {
    IconButton {
        id: id.into(),
        icon,
        label: label.into(),
        keys: None,
        dense: false,
        toggled: false,
        disabled: false,
        show_hover: false,
        show_focus: false,
        color: None,
        focus: None,
        on_click: None,
        content: None,
    }
}

impl IconButton {
    pub fn keys(mut self, keys: impl Into<SharedString>) -> Self {
        self.keys = Some(keys.into());
        self
    }

    pub fn dense(mut self) -> Self {
        self.dense = true;
        self
    }

    pub fn toggled(mut self, toggled: bool) -> Self {
        self.toggled = toggled;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    pub fn show_hover(mut self) -> Self {
        self.show_hover = true;
        self
    }

    pub fn show_focus(mut self) -> Self {
        self.show_focus = true;
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let size = if self.dense {
            ICON_BUTTON_DENSE
        } else {
            ICON_BUTTON
        };
        let glyph = if self.dense { ICON_SM } else { ICON };
        let color = self.color.unwrap_or(palette.ink).opacity(0.8);
        let disabled = self.disabled;
        let hover = palette.hover;
        let pressed = pressed_fill(&palette);
        let focus_shadows = palette.focus_ring();
        let on_click = self.on_click.clone();
        BaseButton::new(self.id)
            .disabled(disabled)
            .selected(self.toggled)
            .when_some(self.focus.as_ref(), |this, focus| this.track_focus(focus))
            .accessibility_label(self.label.clone())
            .flex_none()
            .size(px(size))
            .rounded(px(if self.dense { RADIUS } else { RADIUS_LG }))
            .text_color(color)
            .cursor_pointer()
            .occlude()
            .when(self.show_hover, |this| this.bg(hover))
            .when(self.show_focus, |this| this.shadow(focus_shadows.clone()))
            .focus_visible(move |style| style.shadow(focus_shadows))
            .styles(|styles| {
                styles
                    .selected(|style| style.bg(pressed))
                    .disabled(|style| style.opacity(0.64))
            })
            .when(!disabled && !self.toggled, |this| {
                this.hover(move |style| style.bg(hover))
                    .active(move |style| style.bg(hover))
            })
            .when(self.toggled, |this| {
                this.aria_toggled(gpui_kit::accesskit::Toggled::True)
            })
            .tooltip(tooltip(self.label.clone(), self.keys.as_deref()))
            .tooltip_show_delay(tooltip_delay())
            .when_some(on_click, |this, handler| {
                this.on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    handler(event, window, cx)
                })
            })
            .flex()
            .items_center()
            .justify_center()
            .child(
                self.content
                    .unwrap_or_else(|| sized_icon(self.icon, glyph, color).into_any_element()),
            )
    }
}

/// A vertical scroll area with a visible, draggable scrollbar, in the shape of
/// shadcn's ScrollArea: the element itself scrolls, and a thin pill thumb sits
/// over its trailing edge whenever the content is taller than the area. The
/// thumb can be dragged, and clicking the track pages to that point.
///
/// Call it last in the element's chain: it wraps the element in a frame that
/// takes over the element's outer size and flex settings, and lays the
/// scrollbar over that frame. The scrollbar must sit outside the scrolled
/// element, or it scrolls away with the content.
///
/// Keep the handle on the view (a `ScrollHandle` is cheap to clone) so the
/// position survives re-renders.
pub trait ScrollArea: StatefulInteractiveElement + Styled + IntoElement + 'static {
    fn scroll_area_y(
        mut self,
        handle: &gpui_kit::ScrollHandle,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        use gpui_kit::component::scroll::Scrollbar;
        use gpui_kit::{Overflow, PointRefinement, StyleRefinement, relative};

        let id = self
            .interactivity()
            .element_id
            .clone()
            .unwrap_or_else(|| ElementId::from("scroll-area"));
        let style = self.style();
        let mut frame = StyleRefinement {
            size: style.size.clone(),
            min_size: style.min_size.clone(),
            max_size: style.max_size.clone(),
            flex_grow: style.flex_grow,
            flex_shrink: style.flex_shrink,
            flex_basis: style.flex_basis,
            align_self: style.align_self,
            ..Default::default()
        };
        frame.overflow = PointRefinement {
            x: None,
            y: Some(Overflow::Hidden),
        };
        // The element fills the frame and shrinks to it, so it scrolls
        // instead of growing past a frame whose height is capped.
        style.max_size.height = None;
        style.min_size.height = Some(px(0.).into());
        style.flex_grow = Some(0.);
        style.flex_shrink = Some(1.);
        style.flex_basis = None;
        style.align_self = None;
        let content = self
            .w_full()
            .h(relative(1.))
            .overflow_y_scroll()
            .track_scroll(handle);

        let mut root = div().id((id.clone(), "frame"));
        root.style().refine(&frame);
        root.relative().flex().flex_col().child(content).child(
            div().absolute().inset_0().child(
                Scrollbar::vertical(handle)
                    .id((id, "scrollbar"))
                    .viewport_from_layout(),
            ),
        )
    }
}

impl<E> ScrollArea for E where E: StatefulInteractiveElement + Styled + IntoElement + 'static {}
