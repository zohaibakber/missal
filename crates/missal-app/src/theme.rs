use std::time::Duration;

use gpui_kit::component::{ActiveTheme, Theme, ThemeMode};
use gpui_kit::{
    Animation, App, BoxShadow, Global, Hsla, Rgba, SharedString, Window, WindowAppearance, point,
    px, rgb,
};
use missal_core::FirStatus;

use crate::prefs::ThemeChoice;

pub fn oklch(lightness: f32, chroma: f32, hue_degrees: f32, alpha: f32) -> Hsla {
    let hue = hue_degrees.to_radians();
    let a = chroma * hue.cos();
    let b = chroma * hue.sin();
    let l = (lightness + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m = (lightness - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s = (lightness - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let linear = [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_93 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_4 * s,
        -0.004_196_086 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ];
    let encode = |value: f32| {
        let value = value.clamp(0.0, 1.0);
        if value <= 0.003_130_8 {
            12.92 * value
        } else {
            1.055 * value.powf(1.0 / 2.4) - 0.055
        }
    };
    Rgba {
        r: encode(linear[0]),
        g: encode(linear[1]),
        b: encode(linear[2]),
        a: alpha,
    }
    .into()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub dark: bool,
    pub canvas: Hsla,
    pub surface: Hsla,
    pub raised: Hsla,
    pub sunken: Hsla,
    pub paper: Hsla,
    pub ink: Hsla,
    pub ink_muted: Hsla,
    pub ink_faint: Hsla,
    pub line: Hsla,
    pub line_strong: Hsla,
    pub line_input: Hsla,
    pub hairline: Hsla,
    pub hover: Hsla,
    pub pressed: Hsla,
    pub hover_raised: Hsla,
    pub pressed_raised: Hsla,
    pub action: Hsla,
    pub action_hover: Hsla,
    pub action_pressed: Hsla,
    pub on_action: Hsla,
    pub accent: Hsla,
    pub accent_wash: Hsla,
    pub field_wash: Hsla,
    pub ring: Hsla,
    pub success: Hsla,
    pub success_text: Hsla,
    pub success_wash: Hsla,
    pub danger: Hsla,
    pub danger_text: Hsla,
    pub danger_wash: Hsla,
    pub notice: Hsla,
    pub notice_text: Hsla,
    pub notice_wash: Hsla,
    pub st_open: Hsla,
    pub st_investigating: Hsla,
    pub st_challan: Hsla,
    pub st_closed: Hsla,
    pub scrim: Hsla,
}

impl Global for Palette {}

pub fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

pub fn hex_alpha(value: u32, alpha: f32) -> Hsla {
    hex(value).opacity(alpha)
}

impl Palette {
    pub fn light() -> Palette {
        Palette {
            dark: false,
            canvas: hex(0xf6f6f6),
            surface: hex(0xffffff),
            raised: hex(0xffffff),
            sunken: hex(0xececec),
            paper: hex(0xffffff),
            ink: hex(0x262626),
            ink_muted: hex(0x686868),
            ink_faint: hex(0x8f8f8f),
            line: hex(0xededed),
            line_strong: hex(0xcfcfcf),
            line_input: hex(0xe3e3e3),
            hairline: hex_alpha(0x000000, 0.08),
            hover: hex_alpha(0x000000, 0.04),
            pressed: hex_alpha(0x000000, 0.07),
            hover_raised: hex_alpha(0x000000, 0.04),
            pressed_raised: hex_alpha(0x000000, 0.07),
            action: hex(0x262626),
            action_hover: hex(0x3b3b3b),
            action_pressed: hex(0x474747),
            on_action: hex(0xfafafa),
            accent: hex(0x155dfc),
            accent_wash: hex(0xeff4ff),
            field_wash: hex(0xf1f5fd),
            ring: hex(0xa1a1a1),
            success: hex(0x00bc7d),
            success_text: hex(0x007a55),
            success_wash: hex(0xebfaf3),
            danger: hex(0xfb2c36),
            danger_text: hex(0xc10007),
            danger_wash: hex(0xfff1f1),
            notice: hex(0xfe9a00),
            notice_text: hex(0xbb4d00),
            notice_wash: hex(0xfff6e8),
            st_open: hex(0xfe9a00),
            st_investigating: hex(0x2b7fff),
            st_challan: hex(0x8e51ff),
            st_closed: hex(0xa1a1a1),
            scrim: hex_alpha(0x000000, 0.32),
        }
    }

    pub fn dark() -> Palette {
        Palette {
            dark: true,
            canvas: hex(0x0f0f0f),
            surface: hex(0x161616),
            raised: hex(0x1c1c1c),
            sunken: hex(0x121212),
            paper: hex(0x1f1f1f),
            ink: hex(0xf5f5f5),
            ink_muted: hex(0x8f8f8f),
            ink_faint: hex(0x707070),
            line: hex(0x242424),
            line_strong: hex(0x3d3d3d),
            line_input: hex(0x2c2c2c),
            hairline: hex_alpha(0xffffff, 0.07),
            hover: hex_alpha(0xffffff, 0.05),
            pressed: hex_alpha(0xffffff, 0.09),
            hover_raised: hex_alpha(0xffffff, 0.06),
            pressed_raised: hex_alpha(0xffffff, 0.1),
            action: hex(0xf5f5f5),
            action_hover: hex(0xdedede),
            action_pressed: hex(0xcccccc),
            on_action: hex(0x262626),
            accent: hex(0x51a2ff),
            accent_wash: hex(0x182335),
            field_wash: hex(0x1b2230),
            ring: hex(0x737373),
            success: hex(0x00bc7d),
            success_text: hex(0x00d492),
            success_wash: hex(0x10291f),
            danger: hex(0xf2444c),
            danger_text: hex(0xff6467),
            danger_wash: hex(0x351819),
            notice: hex(0xfe9a00),
            notice_text: hex(0xffb900),
            notice_wash: hex(0x342611),
            st_open: hex(0xffb020),
            st_investigating: hex(0x51a2ff),
            st_challan: hex(0xa684ff),
            st_closed: hex(0x737373),
            scrim: hex_alpha(0x000000, 0.6),
        }
    }

    pub fn status(&self, status: FirStatus) -> Hsla {
        match status {
            FirStatus::Open => self.st_open,
            FirStatus::UnderInvestigation => self.st_investigating,
            FirStatus::ChallanSubmitted => self.st_challan,
            FirStatus::Closed => self.st_closed,
        }
    }

    pub fn get(cx: &App) -> &Palette {
        cx.global::<Palette>()
    }

    fn shadow_tint(&self, alpha: f32) -> Hsla {
        let boost = if self.dark { 6.0 } else { 1.0 };
        hex(0x000000).opacity((alpha * boost).min(0.6))
    }

    pub fn hairline_ring(&self) -> BoxShadow {
        shadow(self.hairline, 0., 0., 1.)
    }

    fn edge(&self) -> BoxShadow {
        if self.dark {
            inset(hex_alpha(0xffffff, 0.06), 1., 0.)
        } else {
            shadow(hex_alpha(0x000000, 0.04), 1., 0., 0.)
        }
    }

    pub fn ring_only(&self) -> Vec<BoxShadow> {
        vec![self.hairline_ring()]
    }

    pub fn shadow_xs(&self) -> Vec<BoxShadow> {
        vec![
            self.hairline_ring(),
            self.edge(),
            shadow_offset(self.shadow_tint(0.05), 1., 2.),
        ]
    }

    pub fn shadow_sm(&self) -> Vec<BoxShadow> {
        vec![
            self.hairline_ring(),
            self.edge(),
            shadow_offset(self.shadow_tint(0.05), 1., 3.),
            shadow(self.shadow_tint(0.05), 1., 2., -1.),
        ]
    }

    pub fn shadow_md(&self) -> Vec<BoxShadow> {
        vec![
            self.hairline_ring(),
            self.edge(),
            shadow(self.shadow_tint(0.06), 4., 8., -1.),
            shadow(self.shadow_tint(0.05), 2., 4., -2.),
        ]
    }

    pub fn shadow_lg(&self) -> Vec<BoxShadow> {
        vec![
            self.hairline_ring(),
            self.edge(),
            shadow(self.shadow_tint(0.08), 12., 24., -4.),
            shadow(self.shadow_tint(0.05), 4., 8., -4.),
        ]
    }

    pub fn shadow_xl(&self) -> Vec<BoxShadow> {
        vec![
            self.hairline_ring(),
            self.edge(),
            shadow(self.shadow_tint(0.12), 24., 48., -12.),
            shadow(self.shadow_tint(0.05), 8., 16., -8.),
        ]
    }

    pub fn thumb_shadow(&self) -> Vec<BoxShadow> {
        if self.dark {
            vec![
                inset(hex_alpha(0xffffff, 0.06), 1., 0.),
                shadow_offset(hex_alpha(0x000000, 0.3), 1., 2.),
            ]
        } else {
            vec![
                shadow(hex_alpha(0x000000, 0.06), 0., 0., 1.),
                shadow_offset(hex_alpha(0x000000, 0.06), 1., 2.),
            ]
        }
    }

    pub fn primary_shadow(&self) -> Vec<BoxShadow> {
        vec![
            inset(
                hex_alpha(0xffffff, if self.dark { 0.5 } else { 0.16 }),
                1.,
                0.,
            ),
            shadow_offset(self.action.opacity(0.24), 1., 2.),
        ]
    }

    pub fn paper_shadow(&self) -> Vec<BoxShadow> {
        vec![
            self.hairline_ring(),
            shadow_offset(self.shadow_tint(0.06), 2., 6.),
            shadow(self.shadow_tint(0.08), 12., 32., -8.),
        ]
    }

    pub fn focus_ring(&self) -> Vec<BoxShadow> {
        vec![
            shadow(self.ring, 0., 0., 3.),
            shadow(self.surface, 0., 0., 1.),
        ]
    }

    pub fn input_focus_ring(&self) -> Vec<BoxShadow> {
        vec![
            shadow(self.ring, 0., 0., 1.),
            shadow(self.ring.opacity(0.24), 0., 0., 4.),
        ]
    }

    pub fn input_shadow(&self) -> Vec<BoxShadow> {
        vec![
            shadow(self.line_input, 0., 0., 1.),
            self.edge(),
            shadow_offset(self.shadow_tint(0.04), 1., 2.),
        ]
    }

    pub fn danger_ring(&self) -> Vec<BoxShadow> {
        vec![
            shadow(self.danger.opacity(0.64), 0., 0., 1.),
            shadow(self.danger.opacity(0.16), 0., 0., 4.),
        ]
    }

    pub fn danger_edge(&self) -> Vec<BoxShadow> {
        vec![shadow(self.danger.opacity(0.36), 0., 0., 1.)]
    }

    pub fn floating(&self) -> Hsla {
        if self.dark {
            hex(0x262626)
        } else {
            hex(0xffffff)
        }
    }

    pub fn control_fill(&self) -> Hsla {
        if self.dark {
            hex(0x1d1d1d)
        } else {
            self.surface
        }
    }
}

fn shadow(color: Hsla, y: f32, blur: f32, spread: f32) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(spread),
        inset: false,
    }
}

fn inset(color: Hsla, y: f32, blur: f32) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.),
        inset: true,
    }
}

fn shadow_offset(color: Hsla, y: f32, blur: f32) -> BoxShadow {
    shadow(color, y, blur, 0.)
}

pub const FONT_SIZE: f32 = 14.0;

pub const HEADER_HEIGHT: f32 = 40.0;
pub const TITLE_BAR_HEIGHT: f32 = HEADER_HEIGHT;
pub const ICON_BUTTON: f32 = 28.0;
pub const ICON_BUTTON_DENSE: f32 = 24.0;
pub const BUTTON_SM: f32 = 28.0;
pub const BUTTON_MD: f32 = 32.0;
pub const INPUT_LATIN: f32 = 32.0;
pub const INPUT_URDU: f32 = 36.0;
pub const CHECKBOX: f32 = 16.0;
pub const SEGMENTED_TRACK: f32 = 30.0;
pub const SEGMENTED_ITEM: f32 = 26.0;
pub const TAB_HEIGHT: f32 = 40.0;
pub const FILTER_ITEM: f32 = 28.0;
pub const TABLE_HEAD: f32 = 36.0;
pub const TABLE_ROW: f32 = 40.0;
pub const ROW_HEIGHT: f32 = TABLE_ROW;
pub const LIST_ITEM: f32 = 40.0;
pub const LIST_ITEM_META: f32 = 44.0;
pub const MENU_ITEM: f32 = 32.0;
pub const KEYCAP: f32 = 20.0;

pub const ICON: f32 = 16.0;
pub const ICON_SM: f32 = 14.0;
pub const ICON_XS: f32 = 12.0;

pub const RADIUS_CHIP: f32 = 4.0;
pub const RADIUS_SM: f32 = 6.0;
pub const RADIUS: f32 = 8.0;
pub const RADIUS_LG: f32 = 10.0;
pub const RADIUS_XL: f32 = 14.0;
pub const RADIUS_2XL: f32 = 18.0;
pub const RADIUS_PAPER: f32 = 2.0;
pub const RADIUS_PILL: f32 = 999.0;
pub const SHELL_INSET: f32 = 8.0;

pub const SPACE: [f32; 10] = [2.0, 4.0, 6.0, 8.0, 12.0, 16.0, 20.0, 24.0, 32.0, 40.0];
pub const GAP_IN_GROUP: f32 = 4.0;
pub const GAP_BETWEEN_GROUPS: f32 = 12.0;
pub const PANEL_PADDING: f32 = 16.0;
pub const PAGE_GUTTER: f32 = 24.0;

pub const NAV_MENU_WIDTH: f32 = 248.0;
pub const PALETTE_WIDTH: f32 = 600.0;
pub const PALETTE_TOP: f32 = 88.0;
pub const DIALOG_WIDTH: f32 = 420.0;
pub const CHEATSHEET_WIDTH: f32 = 880.0;
pub const TOAST_MAX_WIDTH: f32 = 360.0;
pub const CAPTION_RESERVE: f32 = 138.0;

pub const RAIL_WIDTH: f32 = 296.0;
pub const DETAILS_RAIL_WIDTH: f32 = 320.0;

pub const MOTION_FAST: Duration = Duration::from_millis(120);
pub const MOTION_BASE: Duration = Duration::from_millis(180);
pub const MOTION_EXIT: Duration = Duration::from_millis(100);
pub const TOOLTIP_DELAY: Duration = Duration::from_millis(400);

pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

pub fn motion_fast() -> Animation {
    Animation::new(MOTION_FAST).with_easing(ease_out_cubic)
}

pub fn motion_base() -> Animation {
    Animation::new(MOTION_BASE).with_easing(ease_out_cubic)
}

pub fn motion_exit() -> Animation {
    Animation::new(MOTION_EXIT)
}

fn apply_palette(theme: &mut Theme, palette: &Palette) {
    let transparent = theme.transparent;
    let colors = &mut theme.colors;
    colors.background = palette.surface;
    colors.table = palette.surface;
    colors.table_even = palette.surface;
    colors.table_head = palette.surface;
    colors.list = palette.surface;
    colors.list_even = palette.surface;
    colors.list_head = palette.surface;
    colors.title_bar = palette.canvas;
    colors.status_bar = palette.canvas;
    colors.tab_active = palette.surface;
    colors.sidebar = palette.canvas;
    colors.popover = palette.raised;
    colors.foreground = palette.ink;
    colors.popover_foreground = palette.ink;
    colors.accent_foreground = palette.ink;
    colors.secondary_foreground = palette.ink;
    colors.caret = palette.ink;
    colors.sidebar_foreground = palette.ink;
    colors.sidebar_accent_foreground = palette.ink;
    colors.tab_active_foreground = palette.ink;
    colors.muted_foreground = palette.ink_muted;
    colors.table_head_foreground = palette.ink_muted;
    colors.tab_foreground = palette.ink_muted;
    colors.border = palette.line;
    colors.table_row_border = palette.line;
    colors.title_bar_border = palette.line;
    colors.window_border = palette.line;
    colors.status_bar_border = palette.line;
    colors.sidebar_border = palette.line;
    colors.input = palette.line_input;
    colors.accent = palette.hover_raised;
    colors.list_hover = palette.hover;
    colors.table_hover = palette.hover;
    colors.secondary = palette.hover;
    colors.secondary_hover = palette.hover;
    colors.muted = palette.hover;
    colors.skeleton = palette.hover;
    colors.tab_bar = palette.hover;
    colors.group_box = palette.hover;
    colors.sidebar_accent = palette.hover;
    colors.secondary_active = palette.pressed;
    colors.list_active = palette.pressed;
    colors.table_active = palette.pressed;
    colors.primary = palette.action;
    colors.button_primary = palette.action;
    colors.switch = palette.action;
    colors.primary_hover = palette.action_hover;
    colors.button_primary_hover = palette.action_hover;
    colors.primary_active = palette.action_pressed;
    colors.button_primary_active = palette.action_pressed;
    colors.primary_foreground = palette.on_action;
    colors.button_primary_foreground = palette.on_action;
    colors.sidebar_primary = palette.action;
    colors.sidebar_primary_foreground = palette.on_action;
    colors.button_secondary = palette.hover;
    colors.button_secondary_hover = palette.pressed;
    colors.button_secondary_active = palette.pressed;
    colors.button_secondary_foreground = palette.ink;
    colors.ring = palette.ring;
    colors.link = palette.accent;
    colors.drag_border = palette.accent;
    colors.list_active_border = palette.accent;
    colors.table_active_border = palette.accent;
    colors.selection = palette
        .accent
        .opacity(if palette.dark { 0.36 } else { 0.22 });
    colors.drop_target = palette.accent_wash;
    colors.danger = palette.danger;
    colors.button_danger = palette.danger;
    colors.danger_hover = palette.danger.opacity(0.9);
    colors.danger_active = palette.danger.opacity(0.84);
    colors.button_danger_hover = colors.danger_hover;
    colors.button_danger_active = colors.danger_active;
    colors.danger_foreground = hex(0xffffff);
    colors.button_danger_foreground = hex(0xffffff);
    colors.warning = palette.notice;
    colors.info = palette.accent;
    colors.success = palette.success;
    colors.overlay = palette.scrim;
    colors.scrollbar = transparent;
    colors.scrollbar_thumb = palette.ink_muted.opacity(0.28);
    colors.scrollbar_thumb_hover = palette.ink_muted.opacity(0.5);
    colors.switch_thumb = hex(0xffffff);
    colors.slider_thumb = palette.surface;
    colors.progress_bar = palette.action;
    colors.slider_bar = palette.action;
    theme.radius = px(RADIUS);
    theme.radius_lg = px(RADIUS_LG);
    theme.font_size = px(FONT_SIZE);
    theme.font_family = SharedString::from(missal_text::UI_FAMILY);
    theme.shadow = true;
    theme.motion.duration_fast = MOTION_FAST;
    theme.motion.duration_normal = MOTION_BASE;
}

pub fn resolve_dark(choice: ThemeChoice, appearance: WindowAppearance) -> bool {
    match choice {
        ThemeChoice::Dark => true,
        ThemeChoice::Light => false,
        ThemeChoice::System => matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    }
}

pub fn install(dark: bool, cx: &mut App) {
    let palette = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        None,
        cx,
    );
    Theme::update(cx, |theme| apply_palette(theme, &palette));
    cx.set_global(palette);
    cx.refresh_windows();
}

pub fn apply(choice: ThemeChoice, window: &Window, cx: &mut App) {
    let dark = resolve_dark(choice, window.appearance());
    if !cx.has_global::<Palette>() || Palette::get(cx).dark != dark || cx.theme().is_dark() != dark
    {
        install(dark, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(color: Hsla) -> [u8; 3] {
        let rgba: Rgba = color.to_rgb();
        [
            (rgba.r * 255.0).round() as u8,
            (rgba.g * 255.0).round() as u8,
            (rgba.b * 255.0).round() as u8,
        ]
    }

    #[test]
    fn oklch_matches_known_values() {
        assert_eq!(rgb(oklch(1.0, 0.0, 0.0, 1.0)), [255, 255, 255]);
        assert_eq!(rgb(oklch(0.0, 0.0, 0.0, 1.0)), [0, 0, 0]);
        let primary = rgb(oklch(0.52, 0.19, 272.0, 1.0));
        assert!(
            primary[2] > primary[0] && primary[2] > primary[1],
            "{primary:?}"
        );
        let mid = rgb(oklch(0.5, 0.0, 0.0, 1.0));
        assert!((mid[0] as i32 - 99).abs() <= 2, "{mid:?}");
    }

    #[test]
    fn system_follows_the_os() {
        assert!(resolve_dark(ThemeChoice::System, WindowAppearance::Dark));
        assert!(!resolve_dark(ThemeChoice::System, WindowAppearance::Light));
        assert!(resolve_dark(ThemeChoice::Dark, WindowAppearance::Light));
        assert!(!resolve_dark(ThemeChoice::Light, WindowAppearance::Dark));
    }
}
