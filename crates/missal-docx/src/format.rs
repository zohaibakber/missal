use std::fmt::Write;

use missal_core::document::{Align, Style, TextFormat};

use crate::props::{Border, LineRule, Rgb, RunProps, Sides, VertAlign};
use crate::styles::Theme;

pub const URDU_FONT_WINDOWS_LINE_RATIO: f64 = (2570.0 + 900.0) / 2048.0;
const DEFAULT_LINE_RATIO: f64 = 1.15;

const FONT_LINE_RATIOS: [(&str, f64); 12] = [
    ("jameel noori nastaleeq", URDU_FONT_WINDOWS_LINE_RATIO),
    ("jameel noori nastaleeq kasheeda", URDU_FONT_WINDOWS_LINE_RATIO),
    ("times new roman", (1825.0 + 443.0) / 2048.0),
    ("arial", (1854.0 + 434.0) / 2048.0),
    ("calibri", (1950.0 + 550.0) / 2048.0),
    ("calibri light", (1950.0 + 550.0) / 2048.0),
    ("cambria", (1946.0 + 455.0) / 2048.0),
    ("tahoma", (2049.0 + 423.0) / 2048.0),
    ("verdana", (2059.0 + 430.0) / 2048.0),
    ("segoe ui", (2210.0 + 514.0) / 2048.0),
    ("courier new", (1705.0 + 615.0) / 2048.0),
    ("aptos", (1923.0 + 541.0) / 2048.0),
];

pub fn line_ratio(family: Option<&str>) -> f64 {
    let Some(family) = family else {
        return DEFAULT_LINE_RATIO;
    };
    let family = family.trim().to_lowercase();
    FONT_LINE_RATIOS
        .iter()
        .find(|(name, _)| *name == family)
        .map_or(DEFAULT_LINE_RATIO, |(_, ratio)| *ratio)
}

pub fn is_complex(c: char) -> bool {
    matches!(c as u32,
        0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1EE00..=0x1EEFF)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Complex,
    Latin,
    Neutral,
}

pub fn script(c: char) -> Script {
    if is_complex(c) {
        Script::Complex
    } else if c.is_alphabetic() {
        Script::Latin
    } else {
        Script::Neutral
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunFormat {
    pub family: Option<String>,
    pub size_px: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub caps: bool,
    pub color: Option<Rgb>,
    pub background: Option<Rgb>,
    pub vert: VertAlign,
    pub rtl: bool,
    pub letter_spacing_px: Option<f64>,
}

impl RunFormat {
    pub fn resolve(props: &RunProps, complex: bool, theme: &Theme) -> RunFormat {
        let fonts = &props.fonts;
        let family = if complex {
            fonts.cs.clone().or_else(|| {
                fonts
                    .cs_theme
                    .as_deref()
                    .and_then(|name| theme.font(name, props.lang_bidi.as_deref()))
            })
        } else {
            fonts
                .ascii
                .clone()
                .or_else(|| fonts.h_ansi.clone())
                .or_else(|| {
                    fonts
                        .ascii_theme
                        .as_deref()
                        .and_then(|name| theme.font(name, None))
                })
        };
        let half_points = if complex {
            props.size_cs.or(props.size)
        } else {
            props.size
        }
        .unwrap_or(20.0)
        .clamp(1.0, 3276.0);
        let (bold, italic) = if complex {
            (props.bold_cs, props.italic_cs)
        } else {
            (props.bold, props.italic)
        };
        let highlight = props.highlight.flatten();
        let shading = props.shading.flatten();
        RunFormat {
            family: family.filter(|family| !family.trim().is_empty()),
            size_px: half_points * 2.0 / 3.0,
            bold: bold.unwrap_or(false),
            italic: italic.unwrap_or(false),
            underline: props.underline.unwrap_or(false),
            strike: props.strike.unwrap_or(false) || props.dstrike.unwrap_or(false),
            caps: props.caps.unwrap_or(false),
            color: props.color.flatten().filter(|color| !color.is_black()),
            background: highlight.or(shading).filter(|color| !color.is_white()),
            vert: props.vert_align.unwrap_or(VertAlign::Baseline),
            rtl: props.rtl.unwrap_or(false),
            letter_spacing_px: props
                .spacing
                .filter(|spacing| *spacing != 0.0)
                .map(|spacing| spacing / 15.0),
        }
    }

    pub fn text_format(&self) -> TextFormat {
        let mut format = TextFormat::default();
        format.set(TextFormat::BOLD, self.bold);
        format.set(TextFormat::ITALIC, self.italic);
        format.set(TextFormat::UNDERLINE, self.underline);
        format.set(TextFormat::STRIKETHROUGH, self.strike);
        format.set(TextFormat::SUPERSCRIPT, self.vert == VertAlign::Super);
        format.set(TextFormat::SUBSCRIPT, self.vert == VertAlign::Sub);
        format.set(TextFormat::UPPERCASE, self.caps);
        format
    }

    pub fn css(&self, line: Line) -> String {
        let mut css = Css::default();
        if let Some(family) = &self.family {
            css.push("font-family", &family_css(family));
        }
        css.push("font-size", &px(self.size_px));
        css.push("font-weight", if self.bold { "700" } else { "400" });
        if self.italic {
            css.push("font-style", "italic");
        }
        css.push("line-height", &px(round2(line.height(self.family.as_deref(), self.size_px))));
        if let Some(color) = self.color {
            css.push("color", &color.css());
        }
        if let Some(background) = self.background {
            css.push("background-color", &background.css());
        }
        let decoration = match (self.underline, self.strike) {
            (true, true) => Some("underline line-through"),
            (true, false) => Some("underline"),
            (false, true) => Some("line-through"),
            (false, false) => None,
        };
        if let Some(decoration) = decoration {
            css.push("text-decoration", decoration);
        }
        match self.vert {
            VertAlign::Super => css.push("vertical-align", "super"),
            VertAlign::Sub => css.push("vertical-align", "sub"),
            VertAlign::Baseline => {}
        }
        if let Some(spacing) = self.letter_spacing_px {
            css.push("letter-spacing", &px(spacing));
        }
        if self.rtl {
            css.push("direction", "rtl");
        }
        css.finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub value: f64,
    pub rule: LineRule,
}

impl Line {
    pub fn from(line: Option<(f64, LineRule)>) -> Line {
        match line {
            Some((value, rule)) if value > 0.0 => Line { value, rule },
            _ => Line {
                value: 240.0,
                rule: LineRule::Auto,
            },
        }
    }

    pub fn height(self, family: Option<&str>, size_px: f64) -> f64 {
        let single = line_ratio(family) * size_px;
        match self.rule {
            LineRule::Auto => self.value / 240.0 * single,
            LineRule::Exact => self.value / 15.0,
            LineRule::AtLeast => (self.value / 15.0).max(single),
        }
    }
}

pub fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

pub fn number(value: f64) -> String {
    let rounded = (value * 10000.0).round() / 10000.0;
    let rounded = if rounded == 0.0 { 0.0 } else { rounded };
    let mut text = format!("{rounded:.4}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

pub fn px(value: f64) -> String {
    format!("{}px", number(value))
}

pub fn pt(value: f64) -> String {
    format!("{}pt", number(value))
}

pub fn family_css(family: &str) -> String {
    let family = family.trim();
    let bare = family
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !family.starts_with(|c: char| c.is_ascii_digit());
    if bare {
        family.to_owned()
    } else {
        format!("\"{}\"", family.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

pub fn align_css(align: Align) -> &'static str {
    match align {
        Align::Left => "left",
        Align::Start => "start",
        Align::Center => "center",
        Align::Right => "right",
        Align::End => "end",
        Align::Justify => "justify",
    }
}

#[derive(Default)]
pub struct Css {
    text: String,
}

impl Css {
    pub fn push(&mut self, property: &str, value: &str) {
        if !self.text.is_empty() {
            self.text.push(' ');
        }
        let _ = write!(self.text, "{property}: {value};");
    }

    pub fn borders(&mut self, sides: [Option<Border>; 4]) {
        if sides.iter().all(Option::is_none) {
            return;
        }
        let widths: Vec<String> = sides
            .iter()
            .map(|side| side.map_or_else(|| "0px".to_owned(), |border| px(round2(border.width_px))))
            .collect();
        let styles: Vec<&str> = sides
            .iter()
            .map(|side| side.map_or("none", |border| border.style))
            .collect();
        let colors: Vec<String> = sides
            .iter()
            .map(|side| side.and_then(|border| border.color).unwrap_or(Rgb(0, 0, 0)).css())
            .collect();
        self.push("border-width", &four(&widths));
        self.push("border-style", &four(&styles));
        self.push("border-color", &four(&colors));
    }

    pub fn finish(self) -> String {
        self.text
    }

    pub fn style(self) -> Option<Style> {
        (!self.text.is_empty()).then(|| Style::new(&self.text))
    }
}

pub fn four<T: AsRef<str> + PartialEq>(values: &[T]) -> String {
    let [top, right, bottom, left] = values else {
        return String::new();
    };
    let parts: Vec<&str> = if top == right && top == bottom && top == left {
        vec![top.as_ref()]
    } else if top == bottom && right == left {
        vec![top.as_ref(), right.as_ref()]
    } else if right == left {
        vec![top.as_ref(), right.as_ref(), bottom.as_ref()]
    } else {
        vec![top.as_ref(), right.as_ref(), bottom.as_ref(), left.as_ref()]
    };
    parts.join(" ")
}

pub fn physical<T: Copy>(sides: &Sides<T>, rtl: bool) -> [Option<T>; 4] {
    let (left, right) = if rtl {
        (sides.end, sides.start)
    } else {
        (sides.start, sides.end)
    };
    [sides.top, right, sides.bottom, left]
}
