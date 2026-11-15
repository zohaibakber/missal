use gpui_kit::component::ActiveTheme;
use gpui_kit::{App, BoxShadow, Global, Hsla, point, px};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorTheme {
    pub surface: Hsla,
    pub raised: Hsla,
    pub canvas: Hsla,
    pub paper: Hsla,
    pub ink: Hsla,
    pub ink_muted: Hsla,
    pub ink_faint: Hsla,
    pub line: Hsla,
    pub line_strong: Hsla,
    pub hover: Hsla,
    pub pressed: Hsla,
    pub accent: Hsla,
    pub field_wash: Hsla,
    pub notice: Hsla,
    pub notice_text: Hsla,
    pub notice_wash: Hsla,
    pub selection: Hsla,
    pub shadow: Hsla,
}

impl Global for EditorTheme {}

impl EditorTheme {
    pub fn get(cx: &App) -> EditorTheme {
        if let Some(theme) = cx.try_global::<EditorTheme>() {
            return *theme;
        }
        let theme = cx.theme();
        EditorTheme {
            surface: theme.background,
            raised: theme.popover,
            canvas: theme.muted,
            paper: gpui_kit::white(),
            ink: theme.foreground,
            ink_muted: theme.muted_foreground,
            ink_faint: theme.muted_foreground,
            line: theme.border,
            line_strong: theme.input,
            hover: theme.accent,
            pressed: theme.secondary_active,
            accent: theme.ring,
            field_wash: theme.ring.opacity(0.1),
            notice: theme.warning,
            notice_text: theme.warning,
            notice_wash: theme.warning.opacity(0.15),
            selection: theme.selection,
            shadow: gpui_kit::black().opacity(0.05),
        }
    }

    pub fn install(self, cx: &mut App) {
        cx.set_global(self);
    }

    pub fn paper_shadows(&self) -> [BoxShadow; 2] {
        [
            BoxShadow {
                color: self.line,
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: px(1.),
                inset: false,
            },
            BoxShadow {
                color: self.shadow,
                offset: point(px(0.), px(1.)),
                blur_radius: px(2.),
                spread_radius: px(0.),
                inset: false,
            },
        ]
    }

    pub fn control_shadow(&self) -> BoxShadow {
        BoxShadow {
            color: self.shadow,
            offset: point(px(0.), px(1.)),
            blur_radius: px(2.),
            spread_radius: px(0.),
            inset: false,
        }
    }
}
