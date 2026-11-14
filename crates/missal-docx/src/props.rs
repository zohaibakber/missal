use roxmltree::Node;

use crate::xml::{attr, child_val, measure, number, on_off, twips, val, w_name};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn parse(hex: &str) -> Option<Rgb> {
        let hex = hex.trim().trim_start_matches('#');
        if hex.len() != 6 || hex.eq_ignore_ascii_case("auto") {
            return None;
        }
        let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
        Some(Rgb(channel(0)?, channel(2)?, channel(4)?))
    }

    pub fn highlight(name: &str) -> Option<Rgb> {
        Some(match name {
            "black" => Rgb(0, 0, 0),
            "blue" => Rgb(0, 0, 255),
            "cyan" => Rgb(0, 255, 255),
            "green" => Rgb(0, 255, 0),
            "magenta" => Rgb(255, 0, 255),
            "red" => Rgb(255, 0, 0),
            "yellow" => Rgb(255, 255, 0),
            "white" => Rgb(255, 255, 255),
            "darkBlue" => Rgb(0, 0, 139),
            "darkCyan" => Rgb(0, 139, 139),
            "darkGreen" => Rgb(0, 100, 0),
            "darkMagenta" => Rgb(139, 0, 139),
            "darkRed" => Rgb(139, 0, 0),
            "darkYellow" => Rgb(128, 128, 0),
            "darkGray" => Rgb(169, 169, 169),
            "lightGray" => Rgb(211, 211, 211),
            _ => return None,
        })
    }

    pub fn css(self) -> String {
        format!("rgb({}, {}, {})", self.0, self.1, self.2)
    }

    pub fn is_white(self) -> bool {
        self == Rgb(255, 255, 255)
    }

    pub fn is_black(self) -> bool {
        self == Rgb(0, 0, 0)
    }
}

pub fn shading(node: Node) -> Option<Rgb> {
    let fill = attr(node, "fill").and_then(Rgb::parse);
    let pattern = val(node).unwrap_or("clear");
    let color = attr(node, "color").and_then(Rgb::parse);
    match pattern {
        "nil" => None,
        "solid" => color.or(Some(Rgb(0, 0, 0))),
        "clear" => fill,
        _ => fill.or(color),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertAlign {
    Baseline,
    Super,
    Sub,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fonts {
    pub ascii: Option<String>,
    pub h_ansi: Option<String>,
    pub cs: Option<String>,
    pub ascii_theme: Option<String>,
    pub cs_theme: Option<String>,
    pub hint_cs: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunProps {
    pub style: Option<String>,
    pub fonts: Fonts,
    pub size: Option<f64>,
    pub size_cs: Option<f64>,
    pub bold: Option<bool>,
    pub bold_cs: Option<bool>,
    pub italic: Option<bool>,
    pub italic_cs: Option<bool>,
    pub caps: Option<bool>,
    pub small_caps: Option<bool>,
    pub strike: Option<bool>,
    pub dstrike: Option<bool>,
    pub vanish: Option<bool>,
    pub underline: Option<bool>,
    pub color: Option<Option<Rgb>>,
    pub highlight: Option<Option<Rgb>>,
    pub shading: Option<Option<Rgb>>,
    pub vert_align: Option<VertAlign>,
    pub rtl: Option<bool>,
    pub cs: Option<bool>,
    pub lang_bidi: Option<String>,
    pub spacing: Option<f64>,
}

macro_rules! overlay_fields {
    ($target:expr, $source:expr, $($field:ident),*) => {
        $(if $source.$field.is_some() { $target.$field = $source.$field.clone(); })*
    };
}

impl RunProps {
    pub fn parse(node: Node) -> RunProps {
        let mut props = RunProps::default();
        for element in node.children() {
            let Some(name) = w_name(element) else {
                continue;
            };
            match name {
                "rStyle" => props.style = val(element).map(str::to_owned),
                "rFonts" => {
                    let font = |name: &str| attr(element, name).map(str::to_owned);
                    props.fonts = Fonts {
                        ascii: font("ascii"),
                        h_ansi: font("hAnsi"),
                        cs: font("cs"),
                        ascii_theme: font("asciiTheme").or_else(|| font("hAnsiTheme")),
                        cs_theme: font("cstheme"),
                        hint_cs: attr(element, "hint").map(|hint| hint == "cs"),
                    };
                }
                "sz" => props.size = number(val(element)),
                "szCs" => props.size_cs = number(val(element)),
                "b" => props.bold = Some(on_off(element)),
                "bCs" => props.bold_cs = Some(on_off(element)),
                "i" => props.italic = Some(on_off(element)),
                "iCs" => props.italic_cs = Some(on_off(element)),
                "caps" => props.caps = Some(on_off(element)),
                "smallCaps" => props.small_caps = Some(on_off(element)),
                "strike" => props.strike = Some(on_off(element)),
                "dstrike" => props.dstrike = Some(on_off(element)),
                "vanish" | "specVanish" => props.vanish = Some(on_off(element)),
                "u" => props.underline = Some(!matches!(val(element), Some("none"))),
                "color" => props.color = Some(val(element).and_then(Rgb::parse)),
                "highlight" => props.highlight = Some(val(element).and_then(Rgb::highlight)),
                "shd" => props.shading = Some(shading(element)),
                "vertAlign" => {
                    props.vert_align = Some(match val(element) {
                        Some("superscript") => VertAlign::Super,
                        Some("subscript") => VertAlign::Sub,
                        _ => VertAlign::Baseline,
                    })
                }
                "rtl" => props.rtl = Some(on_off(element)),
                "cs" => props.cs = Some(on_off(element)),
                "lang" => props.lang_bidi = attr(element, "bidi").map(str::to_owned),
                "spacing" => props.spacing = number(val(element)),
                _ => {}
            }
        }
        props
    }

    pub fn overlay(&mut self, other: &RunProps) {
        self.overlay_fonts(&other.fonts);
        overlay_fields!(
            self, other, style, size, size_cs, bold, bold_cs, italic, italic_cs, caps, small_caps,
            strike, dstrike, vanish, underline, color, highlight, shading, vert_align, rtl, cs,
            lang_bidi, spacing
        );
    }

    pub fn toggle_overlay(&mut self, other: &RunProps) {
        let toggles = [
            (&mut self.bold, other.bold),
            (&mut self.bold_cs, other.bold_cs),
            (&mut self.italic, other.italic),
            (&mut self.italic_cs, other.italic_cs),
            (&mut self.caps, other.caps),
            (&mut self.small_caps, other.small_caps),
            (&mut self.strike, other.strike),
            (&mut self.dstrike, other.dstrike),
            (&mut self.vanish, other.vanish),
        ];
        for (target, source) in toggles {
            if source == Some(true) {
                *target = Some(!target.unwrap_or(false));
            }
        }
        self.overlay_fonts(&other.fonts);
        overlay_fields!(
            self, other, size, size_cs, underline, color, highlight, shading, vert_align, rtl, cs,
            lang_bidi, spacing
        );
    }

    fn overlay_fonts(&mut self, fonts: &Fonts) {
        let target = &mut self.fonts;
        if fonts.ascii.is_some() {
            target.ascii = fonts.ascii.clone();
            target.ascii_theme = None;
        }
        if fonts.h_ansi.is_some() {
            target.h_ansi = fonts.h_ansi.clone();
        }
        if fonts.cs.is_some() {
            target.cs = fonts.cs.clone();
            target.cs_theme = None;
        }
        if fonts.ascii_theme.is_some() {
            target.ascii_theme = fonts.ascii_theme.clone();
        }
        if fonts.cs_theme.is_some() {
            target.cs_theme = fonts.cs_theme.clone();
        }
        if fonts.hint_cs.is_some() {
            target.hint_cs = fonts.hint_cs;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Jc {
    Start,
    End,
    Center,
    Justify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineRule {
    Auto,
    Exact,
    AtLeast,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Border {
    pub width_px: f64,
    pub color: Option<Rgb>,
    pub style: &'static str,
}

impl Border {
    pub fn parse(node: Node) -> Option<Border> {
        let kind = val(node).unwrap_or("single");
        if matches!(kind, "nil" | "none") {
            return None;
        }
        let eighths = number(attr(node, "sz")).unwrap_or(4.0).max(2.0);
        let style = match kind {
            "double" | "triple" => "double",
            "dotted" | "dotDotDash" => "dotted",
            "dashed" | "dashSmallGap" | "dotDash" | "dashDotStroked" => "dashed",
            "inset" => "inset",
            "outset" => "outset",
            "threeDEmboss" => "ridge",
            "threeDEngrave" => "groove",
            _ => "solid",
        };
        Some(Border {
            width_px: eighths / 6.0,
            color: attr(node, "color").and_then(Rgb::parse),
            style,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Sides<T> {
    pub top: Option<T>,
    pub bottom: Option<T>,
    pub start: Option<T>,
    pub end: Option<T>,
    pub inside_h: Option<T>,
    pub inside_v: Option<T>,
}

impl<T: Clone> Sides<T> {
    pub fn overlay(&mut self, other: &Sides<T>) {
        overlay_fields!(self, other, top, bottom, start, end, inside_h, inside_v);
    }
}

pub fn borders(node: Node) -> Sides<Option<Border>> {
    let mut sides = Sides::default();
    for element in node.children() {
        let parsed = Some(Border::parse(element));
        match w_name(element) {
            Some("top") => sides.top = parsed,
            Some("bottom") => sides.bottom = parsed,
            Some("left" | "start") => sides.start = parsed,
            Some("right" | "end") => sides.end = parsed,
            Some("insideH") => sides.inside_h = parsed,
            Some("insideV") => sides.inside_v = parsed,
            _ => {}
        }
    }
    sides
}

pub fn margins(node: Node) -> Sides<f64> {
    let mut sides = Sides::default();
    for element in node.children() {
        if attr(element, "type").is_some_and(|kind| kind != "dxa") {
            continue;
        }
        let width = twips(element, "w");
        match w_name(element) {
            Some("top") => sides.top = width,
            Some("bottom") => sides.bottom = width,
            Some("left" | "start") => sides.start = width,
            Some("right" | "end") => sides.end = width,
            _ => {}
        }
    }
    sides
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParaProps {
    pub style: Option<String>,
    pub jc: Option<Jc>,
    pub bidi: Option<bool>,
    pub ind_start: Option<f64>,
    pub ind_end: Option<f64>,
    pub ind_first: Option<f64>,
    pub before: Option<f64>,
    pub after: Option<f64>,
    pub before_auto: Option<bool>,
    pub after_auto: Option<bool>,
    pub line: Option<(f64, LineRule)>,
    pub page_break_before: Option<bool>,
    pub contextual_spacing: Option<bool>,
    pub numbering: Option<(Option<String>, Option<u8>)>,
    pub shading: Option<Option<Rgb>>,
    pub borders: Sides<Option<Border>>,
}

impl ParaProps {
    pub fn parse(node: Node) -> ParaProps {
        let mut props = ParaProps::default();
        for element in node.children() {
            let Some(name) = w_name(element) else {
                continue;
            };
            match name {
                "pStyle" => props.style = val(element).map(str::to_owned),
                "jc" => {
                    props.jc = match val(element) {
                        Some("left" | "start") => Some(Jc::Start),
                        Some("right" | "end") => Some(Jc::End),
                        Some("center") => Some(Jc::Center),
                        Some(
                            "both" | "distribute" | "lowKashida" | "mediumKashida" | "highKashida"
                            | "thaiDistribute",
                        ) => Some(Jc::Justify),
                        _ => props.jc,
                    }
                }
                "bidi" => props.bidi = Some(on_off(element)),
                "ind" => {
                    props.ind_start = twips(element, "start").or(twips(element, "left"));
                    props.ind_end = twips(element, "end").or(twips(element, "right"));
                    if let Some(hanging) = twips(element, "hanging") {
                        props.ind_first = Some(-hanging);
                    } else if let Some(first) = twips(element, "firstLine") {
                        props.ind_first = Some(first);
                    }
                }
                "spacing" => {
                    if let Some(before) = twips(element, "before") {
                        props.before = Some(before);
                    }
                    if let Some(after) = twips(element, "after") {
                        props.after = Some(after);
                    }
                    if let Some(auto) = attr(element, "beforeAutospacing") {
                        props.before_auto = Some(crate::xml::on_off_value(Some(auto)));
                    }
                    if let Some(auto) = attr(element, "afterAutospacing") {
                        props.after_auto = Some(crate::xml::on_off_value(Some(auto)));
                    }
                    if let Some(line) = number(attr(element, "line")) {
                        let rule = match attr(element, "lineRule") {
                            Some("exact") => LineRule::Exact,
                            Some("atLeast") => LineRule::AtLeast,
                            _ => LineRule::Auto,
                        };
                        props.line = Some((line, rule));
                    }
                }
                "pageBreakBefore" => props.page_break_before = Some(on_off(element)),
                "contextualSpacing" => props.contextual_spacing = Some(on_off(element)),
                "numPr" => {
                    let id = child_val(element, "numId").map(str::to_owned);
                    let level = child_val(element, "ilvl").and_then(|level| level.parse().ok());
                    props.numbering = Some((id, level));
                }
                "shd" => props.shading = Some(shading(element)),
                "pBdr" => props.borders = borders(element),
                _ => {}
            }
        }
        props
    }

    pub fn overlay(&mut self, other: &ParaProps) {
        overlay_fields!(
            self,
            other,
            style,
            jc,
            bidi,
            ind_start,
            ind_end,
            ind_first,
            before,
            after,
            before_auto,
            after_auto,
            line,
            page_break_before,
            contextual_spacing,
            shading
        );
        if let Some((id, level)) = &other.numbering {
            let (current_id, current_level) = self.numbering.get_or_insert((None, None));
            if id.is_some() {
                *current_id = id.clone();
            }
            if level.is_some() {
                *current_level = *level;
            }
        }
        self.borders.overlay(&other.borders);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Width {
    Twips(f64),
    Percent(f64),
}

pub fn width(node: Node) -> Option<Width> {
    let raw = attr(node, "w")?;
    match attr(node, "type").unwrap_or("dxa") {
        "dxa" => measure(raw).filter(|width| *width > 0.0).map(Width::Twips),
        "pct" => {
            let value = number(Some(raw))?;
            let percent = if raw.trim().ends_with('%') {
                value
            } else {
                value / 50.0
            };
            (percent > 0.0).then_some(Width::Percent(percent))
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableProps {
    pub style: Option<String>,
    pub bidi: Option<bool>,
    pub width: Option<Option<Width>>,
    pub indent: Option<f64>,
    pub jc: Option<Jc>,
    pub borders: Sides<Option<Border>>,
    pub cell_margins: Sides<f64>,
    pub shading: Option<Option<Rgb>>,
    pub floating: bool,
}

impl TableProps {
    pub fn parse(node: Node) -> TableProps {
        let mut props = TableProps::default();
        for element in node.children() {
            let Some(name) = w_name(element) else {
                continue;
            };
            match name {
                "tblStyle" => props.style = val(element).map(str::to_owned),
                "bidiVisual" => props.bidi = Some(on_off(element)),
                "tblW" => props.width = Some(width(element)),
                "tblInd" => {
                    if attr(element, "type").is_none_or(|kind| kind == "dxa") {
                        props.indent = twips(element, "w");
                    }
                }
                "jc" => {
                    props.jc = match val(element) {
                        Some("center") => Some(Jc::Center),
                        Some("right" | "end") => Some(Jc::End),
                        Some("left" | "start") => Some(Jc::Start),
                        _ => None,
                    }
                }
                "tblBorders" => props.borders = borders(element),
                "tblCellMar" => props.cell_margins = margins(element),
                "shd" => props.shading = Some(shading(element)),
                "tblpPr" => props.floating = true,
                _ => {}
            }
        }
        props
    }

    pub fn overlay(&mut self, other: &TableProps) {
        overlay_fields!(self, other, style, bidi, width, indent, jc, shading);
        self.borders.overlay(&other.borders);
        self.cell_margins.overlay(&other.cell_margins);
        self.floating |= other.floating;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VMerge {
    Restart,
    Continue,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CellProps {
    pub width: Option<Width>,
    pub span: u32,
    pub v_merge: Option<VMerge>,
    pub h_merge: Option<VMerge>,
    pub borders: Sides<Option<Border>>,
    pub margins: Sides<f64>,
    pub shading: Option<Option<Rgb>>,
    pub v_align: Option<&'static str>,
}

impl CellProps {
    pub fn parse(node: Option<Node>) -> CellProps {
        let mut props = CellProps {
            span: 1,
            ..CellProps::default()
        };
        let Some(node) = node else { return props };
        for element in node.children() {
            let Some(name) = w_name(element) else {
                continue;
            };
            match name {
                "tcW" => props.width = width(element),
                "gridSpan" => {
                    props.span =
                        number(val(element)).map_or(1, |span| span.clamp(1.0, 1000.0) as u32)
                }
                "vMerge" => {
                    props.v_merge = Some(match val(element) {
                        Some("restart") => VMerge::Restart,
                        _ => VMerge::Continue,
                    })
                }
                "hMerge" => {
                    props.h_merge = Some(match val(element) {
                        Some("restart") => VMerge::Restart,
                        _ => VMerge::Continue,
                    })
                }
                "tcBorders" => props.borders = borders(element),
                "tcMar" => props.margins = margins(element),
                "shd" => props.shading = Some(shading(element)),
                "vAlign" => {
                    props.v_align = match val(element) {
                        Some("center") => Some("middle"),
                        Some("bottom") => Some("bottom"),
                        Some("top") => Some("top"),
                        _ => None,
                    }
                }
                _ => {}
            }
        }
        props
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RowProps {
    pub height: Option<(f64, bool)>,
    pub grid_before: u32,
    pub grid_after: u32,
    pub before_width: Option<Width>,
    pub after_width: Option<Width>,
}

impl RowProps {
    pub fn parse(node: Option<Node>) -> RowProps {
        let mut props = RowProps::default();
        let Some(node) = node else { return props };
        let count =
            |element: Node| number(val(element)).map_or(0, |count| count.clamp(0.0, 1000.0) as u32);
        for element in node.children() {
            match w_name(element) {
                Some("trHeight") => {
                    if let Some(height) = number(val(element)) {
                        props.height = Some((height, attr(element, "hRule") == Some("exact")));
                    }
                }
                Some("gridBefore") => props.grid_before = count(element),
                Some("gridAfter") => props.grid_after = count(element),
                Some("wBefore") => props.before_width = width(element),
                Some("wAfter") => props.after_width = width(element),
                _ => {}
            }
        }
        props
    }
}
