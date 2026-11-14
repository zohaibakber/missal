use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::h_flex;
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Decorations, Global, Hsla, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, StatefulInteractiveElement, Styled, Window, WindowControlArea, div, px,
};

use crate::theme::{HEADER_HEIGHT, ICON_SM, Palette, hex};
use crate::ui::sized_icon;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ControlsStyle {
    None,
    Windows,
    MacTrafficLights,
    Linux,
}

pub struct ForceControls(pub ControlsStyle);

impl Global for ForceControls {}

pub const CAPTION_BUTTON_WIDTH: f32 = 46.0;
pub const MAC_TRAFFIC_LIGHT_INSET: f32 = 80.0;
pub const MAC_TRAFFIC_LIGHT_POSITION: (f32, f32) = (14.0, 14.0);

pub fn style(window: &Window, cx: &App) -> ControlsStyle {
    if let Some(forced) = cx.try_global::<ForceControls>() {
        return forced.0;
    }
    if window.is_fullscreen() {
        return ControlsStyle::None;
    }
    if cfg!(target_os = "windows") {
        ControlsStyle::Windows
    } else if cfg!(target_os = "macos") {
        ControlsStyle::MacTrafficLights
    } else if matches!(window.window_decorations(), Decorations::Client { .. }) {
        ControlsStyle::Linux
    } else {
        ControlsStyle::None
    }
}

pub fn leading_inset(window: &Window, cx: &App) -> Pixels {
    match style(window, cx) {
        ControlsStyle::MacTrafficLights => px(MAC_TRAFFIC_LIGHT_INSET),
        _ => px(8.),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl Control {
    fn id(self) -> &'static str {
        match self {
            Control::Minimize => "window-minimize",
            Control::Maximize => "window-maximize",
            Control::Restore => "window-restore",
            Control::Close => "window-close",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Control::Minimize => "Minimize",
            Control::Maximize => "Maximize",
            Control::Restore => "Restore Down",
            Control::Close => "Close",
        }
    }

    fn area(self) -> WindowControlArea {
        match self {
            Control::Minimize => WindowControlArea::Min,
            Control::Maximize | Control::Restore => WindowControlArea::Max,
            Control::Close => WindowControlArea::Close,
        }
    }

    fn fluent_glyph(self) -> &'static str {
        match self {
            Control::Minimize => "\u{E921}",
            Control::Maximize => "\u{E922}",
            Control::Restore => "\u{E923}",
            Control::Close => "\u{E8BB}",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Control::Minimize => IconName::WindowMinimize,
            Control::Maximize => IconName::WindowMaximize,
            Control::Restore => IconName::WindowRestore,
            Control::Close => IconName::WindowClose,
        }
    }

    fn act(self, window: &mut Window) {
        match self {
            Control::Minimize => window.minimize_window(),
            Control::Maximize | Control::Restore => window.zoom_window(),
            Control::Close => window.remove_window(),
        }
    }
}

struct CaptionFont(Option<&'static str>);

impl Global for CaptionFont {}

fn caption_font(cx: &mut App) -> Option<&'static str> {
    if let Some(font) = cx.try_global::<CaptionFont>() {
        return font.0;
    }
    let names = cx.text_system().all_font_names();
    let found = ["Segoe Fluent Icons", "Segoe MDL2 Assets"]
        .into_iter()
        .find(|family| names.iter().any(|name| name == family));
    cx.set_global(CaptionFont(found));
    found
}

pub fn render(window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    match style(window, cx) {
        ControlsStyle::Windows => Some(windows_controls(window, cx).into_any_element()),
        ControlsStyle::Linux => Some(linux_controls(window, cx).into_any_element()),
        ControlsStyle::MacTrafficLights | ControlsStyle::None => None,
    }
}

fn maximize_control(window: &Window) -> Control {
    if window.is_maximized() {
        Control::Restore
    } else {
        Control::Maximize
    }
}

fn windows_controls(window: &mut Window, cx: &mut App) -> impl IntoElement {
    let palette = *Palette::get(cx);
    let font = if cfg!(target_os = "windows") {
        caption_font(cx)
    } else {
        None
    };
    let active = window.is_window_active() || cx.has_global::<ForceControls>();
    let glyph_color = if active {
        palette.ink
    } else {
        palette.ink_faint
    };
    let (hover, press) = if palette.dark {
        (hex(0xffffff).opacity(0.0605), hex(0xffffff).opacity(0.0419))
    } else {
        (hex(0x000000).opacity(0.0373), hex(0x000000).opacity(0.0241))
    };
    let close_red = hex(0xc42b1c);
    let controls = [Control::Minimize, maximize_control(window), Control::Close];
    h_flex()
        .id("window-controls")
        .flex_none()
        .h(px(HEADER_HEIGHT))
        .items_start()
        .children(controls.into_iter().map(move |control| {
            let close = control == Control::Close;
            let (hover_bg, press_bg) = if close {
                (close_red, close_red.opacity(0.9))
            } else {
                (hover, press)
            };
            let hover_fg: Hsla = if close { hex(0xffffff) } else { glyph_color };
            let glyph: AnyElement = match font {
                Some(family) => div()
                    .font_family(family)
                    .text_size(px(10.))
                    .line_height(px(10.))
                    .child(control.fluent_glyph())
                    .into_any_element(),
                None => sized_icon(control.icon(), ICON_SM, glyph_color).into_any_element(),
            };
            div()
                .id(control.id())
                .flex_none()
                .w(px(CAPTION_BUTTON_WIDTH))
                .h(px(HEADER_HEIGHT - 1.))
                .flex()
                .items_center()
                .justify_center()
                .text_color(glyph_color)
                .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
                .active(move |s| s.bg(press_bg).text_color(hover_fg))
                .role(gpui_kit::accesskit::Role::Button)
                .aria_label(control.label())
                .window_control_area(control.area())
                .when(!cfg!(target_os = "windows"), |this| {
                    this.occlude()
                        .on_mouse_down(MouseButton::Left, |_, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                        })
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            control.act(window);
                        })
                })
                .child(glyph)
                .test_support()
        }))
}

fn linux_controls(window: &mut Window, cx: &mut App) -> impl IntoElement {
    let palette = *Palette::get(cx);
    let supported = window.window_controls();
    let mut controls = Vec::new();
    if supported.minimize {
        controls.push(Control::Minimize);
    }
    if supported.maximize {
        controls.push(maximize_control(window));
    }
    controls.push(Control::Close);
    let rest = palette.hover;
    let hover = palette.pressed;
    h_flex()
        .id("window-controls")
        .flex_none()
        .h(px(HEADER_HEIGHT))
        .items_center()
        .gap(px(12.))
        .pl(px(8.))
        .pr(px(12.))
        .children(controls.into_iter().map(move |control| {
            div()
                .id(control.id())
                .flex_none()
                .size(px(24.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(rest)
                .hover(move |s| s.bg(hover))
                .occlude()
                .role(gpui_kit::accesskit::Role::Button)
                .aria_label(control.label())
                .on_mouse_down(MouseButton::Left, |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    control.act(window);
                })
                .child(sized_icon(control.icon(), 12., palette.ink))
                .test_support()
        }))
}
