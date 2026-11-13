use gpui_kit::component::{ActiveTheme, Colorize, Theme, ThemeMode};
use gpui_kit::{App, Global, Hsla, Rgba, SharedString, Window, WindowAppearance, px};
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

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub background: Hsla,
    pub foreground: Hsla,
    pub card: Hsla,
    pub popover: Hsla,
    pub primary: Hsla,
    pub primary_foreground: Hsla,
    pub secondary: Hsla,
    pub muted: Hsla,
    pub muted_foreground: Hsla,
    pub placeholder: Hsla,
    pub accent: Hsla,
    pub destructive: Hsla,
    pub border: Hsla,
    pub input: Hsla,
    pub ring: Hsla,
    pub sidebar: Hsla,
    pub sidebar_foreground: Hsla,
    pub sidebar_accent: Hsla,
    pub sidebar_border: Hsla,
    pub canvas: Hsla,
    pub paper: Hsla,
    pub paper_foreground: Hsla,
    pub field: Hsla,
    pub status_open: Hsla,
    pub status_investigating: Hsla,
    pub status_challan: Hsla,
    pub status_closed: Hsla,
}

impl Global for Palette {}

impl Palette {
    pub fn light() -> Palette {
        let n = |l: f32, c: f32| oklch(l, c, 264.0, 1.0);
        Palette {
            dark: false,
            background: n(0.995, 0.001),
            foreground: n(0.21, 0.012),
            card: oklch(1.0, 0.0, 0.0, 1.0),
            popover: oklch(1.0, 0.0, 0.0, 1.0),
            primary: oklch(0.52, 0.19, 272.0, 1.0),
            primary_foreground: oklch(0.99, 0.004, 272.0, 1.0),
            secondary: n(0.955, 0.005),
            muted: n(0.962, 0.004),
            muted_foreground: n(0.5, 0.014),
            placeholder: n(0.66, 0.01),
            accent: n(0.952, 0.005),
            destructive: oklch(0.56, 0.21, 25.0, 1.0),
            border: n(0.918, 0.005),
            input: n(0.885, 0.007),
            ring: oklch(0.6, 0.16, 272.0, 1.0),
            sidebar: n(0.968, 0.004),
            sidebar_foreground: n(0.36, 0.014),
            sidebar_accent: n(0.928, 0.006),
            sidebar_border: n(0.905, 0.006),
            canvas: n(0.955, 0.004),
            paper: oklch(1.0, 0.0, 0.0, 1.0),
            paper_foreground: n(0.2, 0.01),
            field: oklch(0.5, 0.19, 272.0, 1.0),
            status_open: oklch(0.68, 0.16, 60.0, 1.0),
            status_investigating: oklch(0.6, 0.17, 250.0, 1.0),
            status_challan: oklch(0.56, 0.2, 300.0, 1.0),
            status_closed: oklch(0.6, 0.14, 155.0, 1.0),
        }
    }

    pub fn dark() -> Palette {
        let n = |l: f32, c: f32| oklch(l, c, 264.0, 1.0);
        let white = |alpha: f32| oklch(1.0, 0.0, 0.0, alpha);
        Palette {
            dark: true,
            background: n(0.2, 0.006),
            foreground: n(0.95, 0.003),
            card: n(0.225, 0.007),
            popover: n(0.235, 0.008),
            primary: oklch(0.58, 0.17, 274.0, 1.0),
            primary_foreground: oklch(0.99, 0.004, 274.0, 1.0),
            secondary: n(0.26, 0.008),
            muted: n(0.25, 0.008),
            muted_foreground: n(0.69, 0.012),
            placeholder: n(0.52, 0.012),
            accent: n(0.265, 0.009),
            destructive: oklch(0.68, 0.19, 22.0, 1.0),
            border: white(0.08),
            input: white(0.11),
            ring: oklch(0.64, 0.15, 274.0, 1.0),
            sidebar: n(0.165, 0.005),
            sidebar_foreground: n(0.78, 0.008),
            sidebar_accent: white(0.07),
            sidebar_border: white(0.07),
            canvas: n(0.172, 0.005),
            paper: n(0.225, 0.007),
            paper_foreground: n(0.93, 0.004),
            field: oklch(0.74, 0.13, 274.0, 1.0),
            status_open: oklch(0.78, 0.15, 70.0, 1.0),
            status_investigating: oklch(0.7, 0.14, 250.0, 1.0),
            status_challan: oklch(0.7, 0.15, 300.0, 1.0),
            status_closed: oklch(0.73, 0.13, 155.0, 1.0),
        }
    }

    pub fn status(&self, status: FirStatus) -> Hsla {
        match status {
            FirStatus::Open => self.status_open,
            FirStatus::UnderInvestigation => self.status_investigating,
            FirStatus::ChallanSubmitted => self.status_challan,
            FirStatus::Closed => self.status_closed,
        }
    }

    pub fn get(cx: &App) -> &Palette {
        cx.global::<Palette>()
    }
}

pub const RADIUS: f32 = 8.0;
pub const FONT_SIZE: f32 = 14.0;
pub const TITLE_BAR_HEIGHT: f32 = 44.0;
pub const SIDEBAR_WIDTH: f32 = 224.0;
pub const SIDEBAR_ICON_WIDTH: f32 = 48.0;
pub const RAIL_WIDTH: f32 = 288.0;
pub const DETAILS_RAIL_WIDTH: f32 = 288.0;
pub const PANE_HEADER_HEIGHT: f32 = 48.0;
pub const RAIL_HEADER_HEIGHT: f32 = 40.0;
pub const STATUS_BAR_HEIGHT: f32 = 32.0;
pub const ROW_HEIGHT: f32 = 44.0;

fn apply_palette(theme: &mut Theme, palette: &Palette) {
    let transparent = theme.transparent;
    let hover = |base: Hsla, toward: Hsla| base.mix_oklab(toward, 0.12);
    let active = |base: Hsla, toward: Hsla| base.mix_oklab(toward, 0.22);
    let toward_ink = if palette.dark { palette.foreground } else { palette.foreground };
    let colors = &mut theme.colors;
    colors.background = palette.background;
    colors.foreground = palette.foreground;
    colors.border = palette.border;
    colors.input = palette.input;
    colors.ring = palette.ring;
    colors.caret = palette.foreground;
    colors.selection = palette.primary.opacity(if palette.dark { 0.4 } else { 0.28 });
    colors.muted = palette.muted;
    colors.muted_foreground = palette.muted_foreground;
    colors.accent = palette.accent;
    colors.accent_foreground = palette.foreground;
    colors.popover = palette.popover;
    colors.popover_foreground = palette.foreground;
    colors.primary = palette.primary;
    colors.primary_hover = hover(palette.primary, palette.primary_foreground);
    colors.primary_active = active(palette.primary, palette.primary_foreground);
    colors.primary_foreground = palette.primary_foreground;
    colors.secondary = palette.secondary;
    colors.secondary_hover = hover(palette.secondary, toward_ink);
    colors.secondary_active = active(palette.secondary, toward_ink);
    colors.secondary_foreground = palette.foreground;
    colors.danger = palette.destructive;
    colors.danger_hover = hover(palette.destructive, palette.primary_foreground);
    colors.danger_active = active(palette.destructive, palette.primary_foreground);
    colors.danger_foreground = palette.primary_foreground;
    colors.button_primary = colors.primary;
    colors.button_primary_hover = colors.primary_hover;
    colors.button_primary_active = colors.primary_active;
    colors.button_primary_foreground = colors.primary_foreground;
    colors.button_secondary = colors.secondary;
    colors.button_secondary_hover = colors.secondary_hover;
    colors.button_secondary_active = colors.secondary_active;
    colors.button_secondary_foreground = colors.secondary_foreground;
    colors.button_danger = colors.danger;
    colors.button_danger_hover = colors.danger_hover;
    colors.button_danger_active = colors.danger_active;
    colors.button_danger_foreground = colors.danger_foreground;
    colors.warning = palette.status_open;
    colors.success = palette.status_closed;
    colors.info = palette.status_investigating;
    colors.link = palette.primary;
    colors.sidebar = palette.sidebar;
    colors.sidebar_foreground = palette.sidebar_foreground;
    colors.sidebar_accent = palette.sidebar_accent;
    colors.sidebar_accent_foreground = palette.foreground;
    colors.sidebar_border = palette.sidebar_border;
    colors.sidebar_primary = palette.primary;
    colors.sidebar_primary_foreground = palette.primary_foreground;
    colors.title_bar = palette.sidebar;
    colors.title_bar_border = palette.sidebar_border;
    colors.status_bar = palette.background;
    colors.status_bar_border = palette.border;
    colors.list = palette.background;
    colors.list_even = palette.background;
    colors.list_head = palette.muted;
    colors.list_hover = palette.accent;
    colors.list_active = palette.accent;
    colors.list_active_border = palette.primary;
    colors.table = palette.background;
    colors.table_even = palette.background;
    colors.table_head = palette.background;
    colors.table_head_foreground = palette.muted_foreground;
    colors.table_hover = palette.accent;
    colors.table_active = palette.accent;
    colors.table_active_border = palette.primary;
    colors.table_row_border = palette.border;
    colors.skeleton = palette.muted;
    colors.switch = palette.input;
    colors.switch_thumb = palette.background;
    colors.slider_bar = palette.primary;
    colors.slider_thumb = palette.background;
    colors.progress_bar = palette.primary;
    colors.tab_bar = palette.muted;
    colors.tab_active = palette.background;
    colors.tab_active_foreground = palette.foreground;
    colors.tab_foreground = palette.muted_foreground;
    colors.scrollbar = transparent;
    colors.scrollbar_thumb = palette.muted_foreground.opacity(0.35);
    colors.scrollbar_thumb_hover = palette.muted_foreground.opacity(0.6);
    colors.overlay = oklch(0.0, 0.0, 0.0, if palette.dark { 0.6 } else { 0.4 });
    colors.group_box = palette.muted;
    colors.window_border = palette.border;
    colors.drag_border = palette.ring;
    colors.drop_target = palette.accent;
    theme.radius = px(RADIUS);
    theme.radius_lg = px(RADIUS + 3.0);
    theme.font_size = px(FONT_SIZE);
    theme.font_family = SharedString::from(missal_text::UI_FAMILY);
    theme.shadow = false;
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
    let palette = if dark { Palette::dark() } else { Palette::light() };
    Theme::change(if dark { ThemeMode::Dark } else { ThemeMode::Light }, None, cx);
    Theme::update(cx, |theme| apply_palette(theme, &palette));
    cx.set_global(palette);
}

pub fn apply(choice: ThemeChoice, window: &Window, cx: &mut App) {
    let dark = resolve_dark(choice, window.appearance());
    if !cx.has_global::<Palette>() || Palette::get(cx).dark != dark || cx.theme().is_dark() != dark {
        install(dark, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(color: Hsla) -> [u8; 3] {
        let rgba: Rgba = color.to_rgb();
        [(rgba.r * 255.0).round() as u8, (rgba.g * 255.0).round() as u8, (rgba.b * 255.0).round() as u8]
    }

    #[test]
    fn oklch_matches_known_values() {
        assert_eq!(rgb(oklch(1.0, 0.0, 0.0, 1.0)), [255, 255, 255]);
        assert_eq!(rgb(oklch(0.0, 0.0, 0.0, 1.0)), [0, 0, 0]);
        let primary = rgb(oklch(0.52, 0.19, 272.0, 1.0));
        assert!(primary[2] > primary[0] && primary[2] > primary[1], "{primary:?}");
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
