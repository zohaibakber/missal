use std::fmt::Write;

use missal_core::document::{
    Align, Block, Direction, ElementProps, HeadingLevel, Inline, List, ListItemChild, ListTag,
    ListType, Table, TableCell, TextFormat,
};
use missal_core::{FieldCatalog, FieldMarkers, FieldReference};

use super::css::{self, Color, Css, Side};
use super::parts::{Abstract, DEFAULT_FONT, Level, LevelKind, Num, link_id};
use super::xml::{eighths, escape, twips};
use crate::format::line_ratio;

const DEFAULT_SIZE_PX: f64 = 20.0;
const CELL_PADDING: [f64; 4] = [4.0, 5.6, 4.0, 5.6];

pub struct Body<'a> {
    catalog: &'a FieldCatalog,
    markers: &'a FieldMarkers,
    pub out: String,
    pub links: Vec<String>,
    pub abstracts: Vec<Abstract>,
    pub nums: Vec<Num>,
    pending_break: bool,
    depth: usize,
    in_link: bool,
    content_twips: i64,
}

struct Para<'d> {
    style: Option<String>,
    props: &'d ElementProps,
    children: Vec<&'d Inline>,
    rtl: bool,
    numbering: Option<(usize, u8)>,
    rule: bool,
}

impl<'d> Para<'d> {
    fn new(props: &'d ElementProps, children: Vec<&'d Inline>, rtl: bool) -> Para<'d> {
        Para {
            style: None,
            props,
            children,
            rtl,
            numbering: None,
            rule: false,
        }
    }
}

#[derive(Clone, Copy)]
struct ListNums {
    plain: usize,
    checked: Option<usize>,
}

enum Entry<'d> {
    Cell {
        column: usize,
        span: usize,
        cell: &'d TableCell,
    },
    Continue {
        column: usize,
        span: usize,
        cell: &'d TableCell,
    },
}

#[derive(Default)]
struct RunLook {
    family: Option<String>,
    half_points: Option<i64>,
    bold: Option<bool>,
    italic: Option<bool>,
    caps: bool,
    strike: bool,
    underline: bool,
    color: Option<Color>,
    background: Option<Color>,
    highlight: bool,
    vert: Option<&'static str>,
    spacing: Option<i64>,
    rtl: bool,
}

fn is_rtl(direction: Direction) -> bool {
    direction == Direction::Rtl
}

fn look(css: &Css, format: TextFormat) -> RunLook {
    let (underline, strike) = css.decoration().unwrap_or((false, false));
    let vert = if format.contains(TextFormat::SUPERSCRIPT) {
        Some("superscript")
    } else if format.contains(TextFormat::SUBSCRIPT) {
        Some("subscript")
    } else {
        match css.get("vertical-align") {
            Some("super") => Some("superscript"),
            Some("sub") => Some("subscript"),
            _ => None,
        }
    };
    RunLook {
        family: css.family(),
        half_points: css
            .font_size()
            .map(|px| (px * 1.5).round().clamp(2.0, 3276.0) as i64),
        bold: if format.contains(TextFormat::BOLD) {
            Some(true)
        } else {
            css.bold()
        },
        italic: if format.contains(TextFormat::ITALIC) {
            Some(true)
        } else {
            css.italic()
        },
        caps: format.contains(TextFormat::UPPERCASE),
        strike: strike || format.contains(TextFormat::STRIKETHROUGH),
        underline: underline || format.contains(TextFormat::UNDERLINE),
        color: css.color("color"),
        background: css
            .color("background-color")
            .or_else(|| css.color("background")),
        highlight: format.contains(TextFormat::HIGHLIGHT),
        vert,
        spacing: css
            .length("letter-spacing", DEFAULT_SIZE_PX)
            .map(twips)
            .filter(|spacing| *spacing != 0),
        rtl: css
            .get("direction")
            .is_some_and(|direction| direction.eq_ignore_ascii_case("rtl")),
    }
}

fn rpr(look: &RunLook) -> String {
    let mut out = String::new();
    if let Some(family) = &look.family {
        let family = escape(family);
        let _ = write!(
            out,
            r#"<w:rFonts w:ascii="{family}" w:hAnsi="{family}" w:eastAsia="{family}" w:cs="{family}"/>"#
        );
    }
    let toggle = |out: &mut String, name: &str, value: Option<bool>| match value {
        Some(true) => {
            let _ = write!(out, "<w:{name}/><w:{name}Cs/>");
        }
        Some(false) => {
            let _ = write!(out, r#"<w:{name} w:val="0"/><w:{name}Cs w:val="0"/>"#);
        }
        None => {}
    };
    toggle(&mut out, "b", look.bold);
    toggle(&mut out, "i", look.italic);
    if look.caps {
        out.push_str("<w:caps/>");
    }
    if look.strike {
        out.push_str("<w:strike/>");
    }
    if let Some(color) = look.color {
        let _ = write!(out, r#"<w:color w:val="{}"/>"#, color.hex());
    }
    if let Some(spacing) = look.spacing {
        let _ = write!(out, r#"<w:spacing w:val="{spacing}"/>"#);
    }
    if let Some(size) = look.half_points {
        let _ = write!(out, r#"<w:sz w:val="{size}"/><w:szCs w:val="{size}"/>"#);
    }
    if look.highlight {
        out.push_str(r#"<w:highlight w:val="yellow"/>"#);
    }
    if look.underline {
        out.push_str(r#"<w:u w:val="single"/>"#);
    }
    if let Some(background) = look.background.filter(|_| !look.highlight) {
        let _ = write!(
            out,
            r#"<w:shd w:val="clear" w:color="auto" w:fill="{}"/>"#,
            background.hex()
        );
    }
    if let Some(vert) = look.vert {
        let _ = write!(out, r#"<w:vertAlign w:val="{vert}"/>"#);
    }
    if look.rtl {
        out.push_str("<w:rtl/>");
    }
    if out.is_empty() {
        out
    } else {
        format!("<w:rPr>{out}</w:rPr>")
    }
}

fn border_kind(style: &str) -> &'static str {
    match style {
        "double" => "double",
        "dotted" => "dotted",
        "dashed" => "dashed",
        "inset" => "inset",
        "outset" => "outset",
        "ridge" => "threeDEmboss",
        "groove" => "threeDEngrave",
        _ => "single",
    }
}

fn border(out: &mut String, name: &str, side: Option<Side>, space: u32) {
    match side {
        Some(side) => {
            let _ = write!(
                out,
                r#"<w:{name} w:val="{}" w:sz="{}" w:space="{space}" w:color="{}"/>"#,
                border_kind(side.style),
                eighths(side.width_px),
                side.color.hex()
            );
        }
        None => {
            let _ = write!(out, r#"<w:{name} w:val="nil"/>"#);
        }
    }
}

fn logical<T: Copy>(physical: [T; 4], rtl: bool) -> [T; 4] {
    let [top, right, bottom, left] = physical;
    if rtl {
        [top, right, bottom, left]
    } else {
        [top, left, bottom, right]
    }
}

fn shading(out: &mut String, color: Color) {
    let _ = write!(
        out,
        r#"<w:shd w:val="clear" w:color="auto" w:fill="{}"/>"#,
        color.hex()
    );
}

fn width_xml(out: &mut String, name: &str, value: Option<&str>, fallback: Option<i64>) {
    if let Some(value) = value {
        if let Some(percent) = value
            .strip_suffix('%')
            .and_then(|n| n.trim().parse::<f64>().ok())
        {
            let _ = write!(
                out,
                r#"<w:{name} w:w="{}" w:type="pct"/>"#,
                (percent * 50.0).round().clamp(0.0, 50_000.0) as i64
            );
            return;
        }
        if let Some(px) = css::length(value, DEFAULT_SIZE_PX) {
            let _ = write!(
                out,
                r#"<w:{name} w:w="{}" w:type="dxa"/>"#,
                twips(px).max(0)
            );
            return;
        }
    }
    match fallback {
        Some(width) => {
            let _ = write!(out, r#"<w:{name} w:w="{width}" w:type="dxa"/>"#);
        }
        None => {
            let _ = write!(out, r#"<w:{name} w:w="0" w:type="auto"/>"#);
        }
    }
}

fn placeholder<'d>(entry: &Entry<'d>) -> Option<(usize, Option<&'d str>)> {
    let Entry::Cell { span, cell, .. } = entry else {
        return None;
    };
    let css = Css::of(cell.props.imported_style.as_ref());
    let blank = match cell.blocks.as_slice() {
        [Block::Paragraph(paragraph)] => {
            paragraph.children.is_empty() && paragraph.props.imported_style.is_none()
        }
        _ => false,
    };
    (blank
        && cell.row_span() == 1
        && css.get("padding") == Some("0px")
        && css.get("vertical-align").is_none())
    .then(|| (*span, css.get("width")))
}

fn level_kind(list: &List) -> LevelKind {
    match list.list_type {
        Some(ListType::Bullet) => LevelKind::Bullet,
        Some(ListType::Number) => LevelKind::Number,
        Some(ListType::Check) => LevelKind::Check,
        None if list.tag == Some(ListTag::Ol) => LevelKind::Number,
        None => LevelKind::Bullet,
    }
}

fn collect_levels(list: &List, level: usize, levels: &mut [Option<Level>; 9]) {
    let index = level.min(8);
    if levels[index].is_none() {
        levels[index] = Some(Level {
            kind: level_kind(list),
            start: list.start.unwrap_or(1).clamp(0, 32_767),
        });
    }
    for item in &list.items {
        for child in &item.children {
            if let ListItemChild::List(nested) = child {
                collect_levels(nested, level + 1, levels);
            }
        }
    }
}

fn has_checked(list: &List) -> bool {
    list.items.iter().any(|item| {
        (level_kind(list) == LevelKind::Check && item.checked == Some(true))
            || item.children.iter().any(|child| match child {
                ListItemChild::List(nested) => has_checked(nested),
                ListItemChild::Inline(_) => false,
            })
    })
}

fn run_line_heights<'d>(
    inlines: &[&'d Inline],
    base: &Css<'d>,
    family: &Option<String>,
    size: f64,
    out: &mut Vec<(Option<String>, f64, f64)>,
) {
    for inline in inlines {
        match inline {
            Inline::Text(text) => {
                let css = base
                    .clone()
                    .with(text.imported_style.as_ref())
                    .with(text.css());
                if let Some(height) = css
                    .get("line-height")
                    .and_then(|value| css::length(value, size))
                {
                    let run_family = css.family().or_else(|| family.clone());
                    let run_size = css.font_size().unwrap_or(size);
                    out.push((run_family, run_size, height));
                }
            }
            Inline::Link(link) => {
                let children: Vec<&Inline> = link.children.iter().collect();
                let css = base.clone().with(link.props.imported_style.as_ref());
                run_line_heights(&children, &css, family, size, out);
            }
            _ => {}
        }
    }
}

fn single(family: Option<&str>, size: f64) -> f64 {
    line_ratio(family) * size
}

fn line_spacing<'d>(
    css: &Css<'d>,
    children: &[&'d Inline],
    family: Option<String>,
    size: Option<f64>,
) -> Option<(i64, &'static str)> {
    let value = css.get("line-height")?;
    if value.eq_ignore_ascii_case("normal") {
        return None;
    }
    let effective_family = family.clone().unwrap_or_else(|| DEFAULT_FONT.to_owned());
    let effective_size = size.unwrap_or(DEFAULT_SIZE_PX);
    let effective_single = single(Some(&effective_family), effective_size);
    let factor = value
        .strip_suffix('%')
        .and_then(|percent| {
            percent
                .trim()
                .parse::<f64>()
                .ok()
                .map(|value| value / 100.0)
        })
        .or_else(|| value.parse::<f64>().ok());
    if let Some(factor) = factor.filter(|factor| factor.is_finite() && *factor > 0.0) {
        let line = factor * effective_size / effective_single * 240.0;
        return Some((line.round().clamp(1.0, 31_680.0) as i64, "auto"));
    }
    let height = css::length(value, effective_size).filter(|height| *height > 0.0)?;
    let mark_size = size.unwrap_or(DEFAULT_SIZE_PX);
    let mut runs = Vec::new();
    run_line_heights(children, &Css::default(), &family, mark_size, &mut runs);
    let base = single(family.as_deref(), mark_size);
    let auto = height * 240.0 / base;
    let exact = (height * 15.0).round();
    let predict = |rule: usize, family: Option<&str>, size: f64| match rule {
        0 => auto.round() / 240.0 * single(family, size),
        1 => exact / 15.0,
        _ => (exact / 15.0).max(single(family, size)),
    };
    let error = |rule: usize| {
        let own = (predict(rule, family.as_deref(), mark_size) - height).abs();
        runs.iter().fold(own, |total, (family, size, run)| {
            total + (predict(rule, family.as_deref(), *size) - run).abs()
        })
    };
    let mut best = 0;
    for rule in 1..3 {
        if error(rule) + 0.01 < error(best) {
            best = rule;
        }
    }
    Some(match best {
        0 => (
            (height * 240.0 / effective_single)
                .round()
                .clamp(1.0, 31_680.0) as i64,
            "auto",
        ),
        1 => (exact.clamp(1.0, 31_680.0) as i64, "exact"),
        _ => (exact.clamp(1.0, 31_680.0) as i64, "atLeast"),
    })
}

fn align_value(align: Align, rtl: bool) -> &'static str {
    match align {
        Align::Start => "left",
        Align::End => "right",
        Align::Center => "center",
        Align::Justify => "both",
        Align::Left if rtl => "right",
        Align::Left => "left",
        Align::Right if rtl => "left",
        Align::Right => "right",
    }
}

fn css_align(value: &str) -> Option<Align> {
    Some(match value.to_ascii_lowercase().as_str() {
        "start" => Align::Start,
        "end" => Align::End,
        "left" | "-webkit-left" => Align::Left,
        "right" | "-webkit-right" => Align::Right,
        "center" | "-webkit-center" => Align::Center,
        "justify" => Align::Justify,
        _ => return None,
    })
}

impl<'a> Body<'a> {
    pub fn new(
        catalog: &'a FieldCatalog,
        markers: &'a FieldMarkers,
        content_twips: i64,
    ) -> Body<'a> {
        Body {
            catalog,
            markers,
            out: String::new(),
            links: Vec::new(),
            abstracts: Vec::new(),
            nums: Vec::new(),
            pending_break: false,
            depth: 0,
            in_link: false,
            content_twips: content_twips.max(1440),
        }
    }

    pub fn finish(&mut self, rtl: bool) {
        if self.pending_break || !self.out.ends_with("</w:p>") {
            self.empty_paragraph(rtl);
        }
    }

    fn empty_paragraph(&mut self, rtl: bool) {
        let page = std::mem::take(&mut self.pending_break) && self.depth == 0;
        self.out.push_str("<w:p><w:pPr>");
        if page {
            self.out.push_str("<w:pageBreakBefore/>");
        }
        self.bidi(rtl);
        self.out.push_str("</w:pPr></w:p>");
    }

    fn bidi(&mut self, rtl: bool) {
        self.out.push_str(if rtl {
            "<w:bidi/>"
        } else {
            r#"<w:bidi w:val="0"/>"#
        });
    }

    pub fn blocks(&mut self, blocks: &[Block], rtl: bool) {
        for block in blocks {
            match block {
                Block::Paragraph(paragraph) => {
                    let children = paragraph.children.iter().collect();
                    self.paragraph(Para::new(&paragraph.props, children, rtl));
                }
                Block::Heading(heading) => {
                    let level = match heading.level.unwrap_or(HeadingLevel::H1) {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };
                    let children = heading.children.iter().collect();
                    let mut para = Para::new(&heading.props, children, rtl);
                    para.style = Some(format!("Heading{level}"));
                    self.paragraph(para);
                }
                Block::Quote(quote) => {
                    let children = quote.children.iter().collect();
                    let mut para = Para::new(&quote.props, children, rtl);
                    para.style = Some("Quote".to_owned());
                    self.paragraph(para);
                }
                Block::List(list) => self.top_list(list, rtl),
                Block::Table(table) => {
                    if self.pending_break {
                        if self.depth == 0 && self.out.ends_with("</w:p>") {
                            self.out.truncate(self.out.len() - "</w:p>".len());
                            self.out
                                .push_str(r#"<w:r><w:br w:type="page"/></w:r></w:p>"#);
                            self.pending_break = false;
                        } else {
                            self.empty_paragraph(rtl);
                        }
                    }
                    self.table(table, rtl);
                }
                Block::PageBreak(_) => {
                    if self.depth == 0 {
                        if self.pending_break {
                            self.empty_paragraph(rtl);
                        }
                        self.pending_break = true;
                    }
                }
                Block::HorizontalRule(leaf) => {
                    let props = ElementProps {
                        imported_style: leaf.imported_style.clone(),
                        ..ElementProps::default()
                    };
                    let mut para = Para::new(&props, Vec::new(), rtl);
                    para.rule = true;
                    self.paragraph(para);
                }
            }
        }
    }

    fn paragraph<'d>(&mut self, para: Para<'d>) {
        let props = para.props;
        let rtl = props.direction.map_or(para.rtl, is_rtl);
        let css = Css::of(props.imported_style.as_ref());
        let empty = para.children.is_empty();
        let (mark_css, mark_format) = if empty {
            (
                css.clone().with(props.text_style.as_ref()),
                props.text_format.unwrap_or_default(),
            )
        } else {
            (css.clone(), TextFormat::default())
        };
        let page = std::mem::take(&mut self.pending_break) && self.depth == 0;
        self.out.push_str("<w:p><w:pPr>");
        if let Some(style) = &para.style {
            let _ = write!(self.out, r#"<w:pStyle w:val="{style}"/>"#);
        }
        if page {
            self.out.push_str("<w:pageBreakBefore/>");
        }
        if let Some((num, level)) = para.numbering {
            let _ = write!(
                self.out,
                r#"<w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="{num}"/></w:numPr>"#
            );
        }
        let borders = if para.rule {
            Some([None, None, Some(Side::solid(2.0)), None])
        } else {
            css.borders()
        };
        if let Some(sides) = borders.filter(|sides| sides.iter().any(Option::is_some)) {
            let [top, start, bottom, end] = logical(sides, rtl);
            self.out.push_str("<w:pBdr>");
            for (name, side) in [
                ("top", top),
                ("left", start),
                ("bottom", bottom),
                ("right", end),
            ] {
                if side.is_some() {
                    border(&mut self.out, name, side, 1);
                }
            }
            self.out.push_str("</w:pBdr>");
        }
        if let Some(color) = css
            .color("background-color")
            .or_else(|| css.color("background"))
        {
            shading(&mut self.out, color);
        }
        self.bidi(rtl);
        let size = mark_css.font_size();
        let margins = css.box_lengths("margin");
        let mut spacing = String::new();
        if para.rule {
            spacing.push_str(r#" w:before="150" w:after="150""#);
        } else if let Some([top, _, bottom, _]) = margins {
            if let Some(top) = top {
                let _ = write!(spacing, r#" w:before="{}""#, twips(top).max(0));
            }
            if let Some(bottom) = bottom {
                let _ = write!(spacing, r#" w:after="{}""#, twips(bottom).max(0));
            }
        }
        if let Some((line, rule)) = line_spacing(&css, &para.children, mark_css.family(), size) {
            let _ = write!(spacing, r#" w:line="{line}" w:lineRule="{rule}""#);
        }
        if !spacing.is_empty() {
            let _ = write!(self.out, "<w:spacing{spacing}/>");
        }
        let extra = props
            .indent
            .filter(|indent| *indent > 0)
            .map_or(0, |indent| twips(f64::from(indent) * 40.0));
        let [_, start, _, end] = margins.map_or([None; 4], |margins| logical(margins, rtl));
        let first = css.length("text-indent", size.unwrap_or(DEFAULT_SIZE_PX));
        let mut indent = String::new();
        if start.is_some() || extra > 0 {
            let _ = write!(
                indent,
                r#" w:left="{}""#,
                twips(start.unwrap_or(0.0)) + extra
            );
        }
        if let Some(end) = end {
            let _ = write!(indent, r#" w:right="{}""#, twips(end));
        }
        match first.map(twips) {
            Some(first) if first < 0 => {
                let _ = write!(indent, r#" w:hanging="{}""#, -first);
            }
            Some(first) => {
                let _ = write!(indent, r#" w:firstLine="{first}""#);
            }
            None => {}
        }
        if !indent.is_empty() {
            let _ = write!(self.out, "<w:ind{indent}/>");
        }
        let align = props
            .align
            .or_else(|| css.get("text-align").and_then(css_align));
        if let Some(align) = align {
            let _ = write!(self.out, r#"<w:jc w:val="{}"/>"#, align_value(align, rtl));
        }
        let mark = rpr(&look(&mark_css, mark_format));
        self.out.push_str(&mark);
        self.out.push_str("</w:pPr>");
        let base = Css::of(props.imported_style.as_ref());
        self.inlines(&para.children, &base);
        self.out.push_str("</w:p>");
    }

    fn inlines<'d>(&mut self, inlines: &[&'d Inline], base: &Css<'d>) {
        for inline in inlines {
            match inline {
                Inline::Text(text) => {
                    let css = base
                        .clone()
                        .with(text.imported_style.as_ref())
                        .with(text.css());
                    let properties = rpr(&look(&css, text.format()));
                    self.text_run(&properties, &text.text);
                }
                Inline::LineBreak(_) => self.out.push_str("<w:r><w:br/></w:r>"),
                Inline::Field(field) => {
                    let css = base.clone().with(field.imported_style.as_ref());
                    let properties = rpr(&look(&css, TextFormat::default()));
                    self.field(&properties, &field.reference);
                }
                Inline::Link(link) => {
                    let css = base.clone().with(link.props.imported_style.as_ref());
                    let children: Vec<&Inline> = link.children.iter().collect();
                    let url = link
                        .url
                        .as_deref()
                        .map(str::trim)
                        .filter(|url| !url.is_empty() && !self.in_link);
                    match url {
                        Some(url) => {
                            if let Some(anchor) = url.strip_prefix('#') {
                                let _ = write!(
                                    self.out,
                                    r#"<w:hyperlink w:anchor="{}" w:history="1">"#,
                                    escape(anchor)
                                );
                            } else {
                                let id = link_id(self.links.len());
                                self.links.push(url.to_owned());
                                let _ =
                                    write!(self.out, r#"<w:hyperlink r:id="{id}" w:history="1">"#);
                            }
                            self.in_link = true;
                            self.inlines(&children, &css);
                            self.in_link = false;
                            self.out.push_str("</w:hyperlink>");
                        }
                        None => self.inlines(&children, &css),
                    }
                }
            }
        }
    }

    fn text_run(&mut self, properties: &str, text: &str) {
        if text.is_empty() {
            return;
        }
        self.out.push_str("<w:r>");
        self.out.push_str(properties);
        let mut pending = String::new();
        let flush = |out: &mut String, pending: &mut String| {
            if !pending.is_empty() {
                let _ = write!(
                    out,
                    r#"<w:t xml:space="preserve">{}</w:t>"#,
                    escape(pending)
                );
                pending.clear();
            }
        };
        for c in text.chars() {
            let element = match c {
                '\t' | '\u{2003}' => "<w:tab/>",
                '\n' | '\u{2028}' => "<w:br/>",
                '\u{2011}' => "<w:noBreakHyphen/>",
                '\r' => continue,
                c => {
                    pending.push(c);
                    continue;
                }
            };
            flush(&mut self.out, &mut pending);
            self.out.push_str(element);
        }
        flush(&mut self.out, &mut pending);
        self.out.push_str("</w:r>");
    }

    fn label(&self, reference: &FieldReference) -> String {
        self.catalog
            .label(reference)
            .unwrap_or_else(|| match reference {
                FieldReference::Catalog(id) => id.to_string(),
                FieldReference::Fir(field) => field.property.as_str().to_owned(),
                FieldReference::Unresolved(text) => text.clone(),
            })
    }

    fn field(&mut self, properties: &str, reference: &FieldReference) {
        let label = self.label(reference);
        let name = label.replace('"', "");
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let instruction = format!(" MERGEFIELD \"{name}\" \\* MERGEFORMAT ");
        let char_run = |out: &mut String, kind: &str| {
            let _ = write!(
                out,
                r#"<w:r>{properties}<w:fldChar w:fldCharType="{kind}"/></w:r>"#
            );
        };
        char_run(&mut self.out, "begin");
        let _ = write!(
            self.out,
            r#"<w:r>{properties}<w:instrText xml:space="preserve">{}</w:instrText></w:r>"#,
            escape(&instruction)
        );
        char_run(&mut self.out, "separate");
        let token = self.markers.token(name);
        self.text_run(properties, &token);
        char_run(&mut self.out, "end");
    }

    fn top_list(&mut self, list: &List, rtl: bool) {
        let mut levels = [None; 9];
        collect_levels(list, 0, &mut levels);
        let abstract_index = self.abstracts.len();
        self.abstracts.push(Abstract { levels });
        self.nums.push(Num {
            abstract_index,
            checked: false,
        });
        let plain = self.nums.len();
        let checked = has_checked(list).then(|| {
            self.nums.push(Num {
                abstract_index,
                checked: true,
            });
            self.nums.len()
        });
        self.list(list, 0, ListNums { plain, checked }, rtl);
    }

    fn list<'d>(&mut self, list: &'d List, level: usize, nums: ListNums, rtl: bool) {
        let rtl = list.props.direction.map_or(rtl, is_rtl);
        let kind = level_kind(list);
        let ilvl = level.min(8) as u8;
        for item in &list.items {
            let item_rtl = item.props.direction.map_or(rtl, is_rtl);
            let num = if kind == LevelKind::Check && item.checked == Some(true) {
                nums.checked.unwrap_or(nums.plain)
            } else {
                nums.plain
            };
            let mut group: Vec<&'d Inline> = Vec::new();
            let mut wrote = false;
            let mut nested_first = false;
            for (index, child) in item.children.iter().enumerate() {
                match child {
                    ListItemChild::Inline(inline) => group.push(inline),
                    ListItemChild::List(nested) => {
                        if !group.is_empty() {
                            self.list_paragraph(
                                item,
                                std::mem::take(&mut group),
                                item_rtl,
                                num,
                                ilvl,
                            );
                            wrote = true;
                        } else if index == 0 {
                            nested_first = true;
                        }
                        self.list(nested, level + 1, nums, item_rtl);
                    }
                }
            }
            if !group.is_empty() || (!wrote && !nested_first) {
                self.list_paragraph(item, group, item_rtl, num, ilvl);
            }
        }
    }

    fn list_paragraph<'d>(
        &mut self,
        item: &'d missal_core::document::ListItem,
        children: Vec<&'d Inline>,
        rtl: bool,
        num: usize,
        level: u8,
    ) {
        let mut para = Para::new(&item.props, children, rtl);
        para.numbering = Some((num, level));
        self.paragraph(para);
    }

    fn place<'d>(table: &'d Table) -> (Vec<Vec<Entry<'d>>>, usize) {
        let mut carry: Vec<u32> = Vec::new();
        let mut origin: Vec<Option<(usize, &'d TableCell)>> = Vec::new();
        let mut columns = 0;
        let mut rows = Vec::with_capacity(table.rows.len());
        for row in &table.rows {
            let mut entries = Vec::new();
            let mut column = 0;
            let mut cells = row.cells.iter();
            loop {
                if column < carry.len() && carry[column] > 0 {
                    match origin[column] {
                        Some((span, cell)) => {
                            entries.push(Entry::Continue { column, span, cell });
                            column += span;
                        }
                        None => column += 1,
                    }
                    continue;
                }
                let Some(cell) = cells.next() else {
                    if carry.iter().skip(column).any(|rows| *rows > 0) {
                        column += 1;
                        continue;
                    }
                    break;
                };
                let span = cell.col_span().clamp(1, 1000) as usize;
                if carry.len() < column + span {
                    carry.resize(column + span, 0);
                    origin.resize(column + span, None);
                }
                let rows_spanned = cell.row_span().clamp(1, 10_000);
                if rows_spanned > 1 {
                    for covered in column..column + span {
                        carry[covered] = rows_spanned;
                        origin[covered] = None;
                    }
                    origin[column] = Some((span, cell));
                }
                entries.push(Entry::Cell { column, span, cell });
                column += span;
            }
            columns = columns.max(column);
            for (index, rows) in carry.iter_mut().enumerate() {
                if *rows > 0 {
                    *rows -= 1;
                    if *rows == 0 {
                        origin[index] = None;
                    }
                }
            }
            rows.push(entries);
        }
        (rows, columns)
    }

    fn grid(&self, table: &Table, columns: usize) -> Vec<i64> {
        let mut grid: Vec<i64> = table
            .col_widths
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|width| twips(*width).max(1))
            .collect();
        if grid.len() < columns {
            let missing = columns - grid.len();
            let used: i64 = grid.iter().sum();
            let fill = ((self.content_twips - used) / missing as i64).max(360);
            grid.extend(std::iter::repeat_n(fill, missing));
        }
        grid
    }

    fn table(&mut self, table: &Table, rtl: bool) {
        let (rows, columns) = Self::place(table);
        if columns == 0 {
            return;
        }
        let rtl = table.props.direction.map_or(rtl, is_rtl);
        let css = Css::of(table.props.imported_style.as_ref());
        let grid = self.grid(table, columns);
        self.depth += 1;
        self.out.push_str("<w:tbl><w:tblPr>");
        if rtl {
            self.out.push_str("<w:bidiVisual/>");
        }
        let imported = table
            .props
            .imported_style
            .as_ref()
            .is_some_and(|style| !style.is_empty());
        let width = css.get("width").or((!imported).then_some("100%"));
        width_xml(&mut self.out, "tblW", width, None);
        let margin = css.get("margin").and_then(css::four);
        let centered = margin.is_some_and(|[_, right, _, left]| right == "auto" && left == "auto");
        if centered {
            self.out.push_str(r#"<w:jc w:val="center"/>"#);
        } else if let Some(indent) = margin
            .map(|[_, right, _, left]| if rtl { right } else { left })
            .and_then(|value| css::length(value, DEFAULT_SIZE_PX))
            .map(twips)
            .filter(|indent| *indent != 0)
        {
            let _ = write!(self.out, r#"<w:tblInd w:w="{indent}" w:type="dxa"/>"#);
        }
        self.out
            .push_str(r#"<w:tblLayout w:type="fixed"/></w:tblPr><w:tblGrid>"#);
        for width in &grid {
            let _ = write!(self.out, r#"<w:gridCol w:w="{width}"/>"#);
        }
        self.out.push_str("</w:tblGrid>");
        let mut header = true;
        for (row, mut entries) in table.rows.iter().zip(rows) {
            header = header && !row.cells.is_empty() && row.cells.iter().all(TableCell::is_header);
            let before = (entries.len() > 1)
                .then(|| placeholder(&entries[0]))
                .flatten();
            if before.is_some() {
                entries.remove(0);
            }
            let after = (entries.len() > 1)
                .then(|| entries.last().and_then(placeholder))
                .flatten();
            if after.is_some() {
                entries.pop();
            }
            self.out.push_str("<w:tr>");
            let row_css = Css::of(row.props.imported_style.as_ref());
            let height = row_css
                .length("height", DEFAULT_SIZE_PX)
                .or(row.height)
                .map(twips)
                .filter(|height| *height > 0);
            if height.is_some() || header || before.is_some() || after.is_some() {
                self.out.push_str("<w:trPr>");
                if let Some((span, _)) = before {
                    let _ = write!(self.out, r#"<w:gridBefore w:val="{span}"/>"#);
                }
                if let Some((span, _)) = after {
                    let _ = write!(self.out, r#"<w:gridAfter w:val="{span}"/>"#);
                }
                if let Some((_, width)) = before {
                    width_xml(&mut self.out, "wBefore", width, None);
                }
                if let Some((_, width)) = after {
                    width_xml(&mut self.out, "wAfter", width, None);
                }
                if let Some(height) = height {
                    let _ = write!(self.out, r#"<w:trHeight w:val="{height}"/>"#);
                }
                if header {
                    self.out.push_str("<w:tblHeader/>");
                }
                self.out.push_str("</w:trPr>");
            }
            if entries.is_empty() {
                self.out.push_str("<w:tc><w:p/></w:tc>");
            }
            for entry in entries {
                match entry {
                    Entry::Cell { column, span, cell } => {
                        let merge = (cell.row_span() > 1).then_some(true);
                        self.cell(cell, &grid, column, span, rtl, merge);
                    }
                    Entry::Continue { column, span, cell } => {
                        self.cell(cell, &grid, column, span, rtl, Some(false));
                    }
                }
            }
            self.out.push_str("</w:tr>");
        }
        self.out.push_str("</w:tbl>");
        self.depth -= 1;
    }

    fn cell(
        &mut self,
        cell: &TableCell,
        grid: &[i64],
        column: usize,
        span: usize,
        rtl: bool,
        merge: Option<bool>,
    ) {
        let css = Css::of(cell.props.imported_style.as_ref());
        self.out.push_str("<w:tc><w:tcPr>");
        let fallback = cell
            .width
            .filter(|width| *width > 0.0)
            .map_or_else(|| grid.iter().skip(column).take(span).sum(), twips);
        width_xml(&mut self.out, "tcW", css.get("width"), Some(fallback));
        if span > 1 {
            let _ = write!(self.out, r#"<w:gridSpan w:val="{span}"/>"#);
        }
        match merge {
            Some(true) => self.out.push_str(r#"<w:vMerge w:val="restart"/>"#),
            Some(false) => self.out.push_str("<w:vMerge/>"),
            None => {}
        }
        let imported = cell
            .props
            .imported_style
            .as_ref()
            .is_some_and(|style| !style.is_empty());
        let sides = css.borders().unwrap_or(if imported {
            [None; 4]
        } else {
            [Some(Side::solid(1.0)); 4]
        });
        let [top, start, bottom, end] = logical(sides, rtl);
        self.out.push_str("<w:tcBorders>");
        for (name, side) in [
            ("top", top),
            ("left", start),
            ("bottom", bottom),
            ("right", end),
        ] {
            border(&mut self.out, name, side, 0);
        }
        self.out.push_str("</w:tcBorders>");
        let background = cell
            .background_color
            .as_deref()
            .and_then(css::color)
            .or_else(|| css.color("background-color"))
            .or_else(|| css.color("background"));
        if let Some(color) = background {
            shading(&mut self.out, color);
        }
        let padding = css.box_lengths("padding").map_or(CELL_PADDING, |sides| {
            let mut resolved = CELL_PADDING;
            for (target, side) in resolved.iter_mut().zip(sides) {
                if let Some(side) = side {
                    *target = side;
                }
            }
            resolved
        });
        let [top, start, bottom, end] = logical(padding, rtl);
        self.out.push_str("<w:tcMar>");
        for (name, value) in [
            ("top", top),
            ("left", start),
            ("bottom", bottom),
            ("right", end),
        ] {
            let _ = write!(
                self.out,
                r#"<w:{name} w:w="{}" w:type="dxa"/>"#,
                twips(value).max(0)
            );
        }
        self.out.push_str("</w:tcMar>");
        let v_align = cell
            .vertical_align
            .as_deref()
            .or_else(|| css.get("vertical-align"))
            .and_then(|value| match value.to_ascii_lowercase().as_str() {
                "middle" | "center" => Some("center"),
                "bottom" => Some("bottom"),
                "top" => Some("top"),
                _ => None,
            });
        if let Some(v_align) = v_align {
            let _ = write!(self.out, r#"<w:vAlign w:val="{v_align}"/>"#);
        }
        self.out.push_str("</w:tcPr>");
        let cell_rtl = cell.props.direction.map_or(rtl, is_rtl);
        if merge != Some(false) {
            self.blocks(&cell.blocks, cell_rtl);
        }
        if !self.out.ends_with("</w:p>") {
            self.empty_paragraph(cell_rtl);
        }
        self.out.push_str("</w:tc>");
    }
}
