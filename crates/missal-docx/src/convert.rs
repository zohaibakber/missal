use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use missal_core::document::{
    Align, Block, Direction, ElementProps, Field, Inline, Leaf, Link, List, ListItem,
    ListItemChild, ListTag, ListType, PageLayout, Paragraph, Style, Table, TableCell, TableRow,
    Text, TextFormat,
};
use missal_core::{FieldCatalog, FieldMarkers, FieldReference};
use roxmltree::Node;

use crate::Notice;
use crate::format::{
    Css, Line, RunFormat, Script, align_css, four, physical, pt, px, round2, script,
};
use crate::package::{Package, Relationships};
use crate::props::{
    Border, CellProps, Jc, ParaProps, Rgb, RowProps, RunProps, Sides, TableProps, VMerge, Width,
};
use crate::styles::{Numbering, Resolved, Styles, Theme};
use crate::xml::{
    M, MC, V, WP, alternate_branch, attr, child, is_ns, is_w, plain_attr, r_attr, text_of, twips,
    val, w_name,
};

pub struct RunStyle {
    normal: RunFormat,
    complex: RunFormat,
    forced: bool,
    hint_cs: bool,
    hidden: bool,
}

impl RunStyle {
    fn format(&self, complex: bool) -> &RunFormat {
        if complex { &self.complex } else { &self.normal }
    }
}

enum Atom {
    Text(String, Rc<RunStyle>),
    LineBreak,
    PageBreak,
    Begin(Rc<RunStyle>),
    Separate,
    End,
    Instr(String),
    Merge(String, Rc<RunStyle>),
    LinkStart(Option<String>),
    LinkEnd,
}

enum Piece {
    Text(String, Rc<RunStyle>),
    Field(FieldReference, Rc<RunStyle>, bool),
    LineBreak,
    PageBreak,
    LinkStart(Option<String>),
    LinkEnd,
}

struct OpenField {
    instruction: String,
    in_result: bool,
    start: usize,
    result_style: Option<Rc<RunStyle>>,
    begin_style: Rc<RunStyle>,
    cross: bool,
}

#[derive(Clone)]
struct Layout {
    rtl: bool,
    align: Align,
    start: f64,
    end: f64,
    first: f64,
    before: f64,
    after: f64,
    contextual: bool,
    style: Option<String>,
    line: Line,
    font: RunFormat,
    shading: Option<Rgb>,
    borders: Sides<Option<Border>>,
}

struct ListInfo {
    group: String,
    level: u8,
    value: i64,
    bullet: bool,
}

struct ParaItem {
    paragraph: Paragraph,
    layout: Layout,
    list: Option<ListInfo>,
}

enum Item {
    Para(Box<ParaItem>),
    Table(Table),
    PageBreak,
    Section(usize),
}

pub struct Section {
    pub layout: PageLayout,
    pub breaks_page: bool,
    pub header: Option<String>,
    pub footer: Option<String>,
}

pub fn section(node: Node) -> Section {
    let size = child(node, "pgSz");
    let margins = child(node, "pgMar");
    let dimension = |node: Option<Node>, name: &str, fallback: f64| {
        node.and_then(|node| twips(node, name))
            .filter(|value| value.is_finite())
            .unwrap_or(fallback)
    };
    let mm = |twips: f64| (twips.abs() / 1440.0 * 25.4 * 1000.0).round() / 1000.0;
    let width = dimension(size, "w", 12240.0);
    let height = dimension(size, "h", 15840.0);
    let gutter = dimension(margins, "gutter", 0.0);
    let mut left = dimension(margins, "left", 1440.0);
    let mut right = dimension(margins, "right", 1440.0);
    let mut top = dimension(margins, "top", 1440.0).abs();
    if gutter > 0.0 {
        if child(node, "rtlGutter").is_some_and(crate::xml::on_off) {
            right += gutter;
        } else if child_on(node, "gutterAtTop") {
            top += gutter;
        } else {
            left += gutter;
        }
    }
    let layout = PageLayout::new(
        mm(width).max(1.0),
        mm(height).max(1.0),
        [
            mm(top),
            mm(right),
            mm(dimension(margins, "bottom", 1440.0)),
            mm(left),
        ],
    )
    .unwrap_or_else(PageLayout::a4);
    let breaks_page = !matches!(
        child(node, "type").and_then(val),
        Some("continuous" | "nextColumn")
    );
    let title_page = child_on(node, "titlePg");
    let reference = |name: &str| {
        let mut chosen = None;
        for item in node.children().filter(|item| is_w(*item, name)) {
            let kind = attr(item, "type").unwrap_or("default");
            let id = r_attr(item, "id").map(str::to_owned);
            if kind == "first" && title_page {
                return id;
            }
            if kind == "default" {
                chosen = id;
            }
        }
        chosen
    };
    Section {
        layout,
        breaks_page,
        header: reference("headerReference"),
        footer: reference("footerReference"),
    }
}

fn child_on(node: Node, name: &str) -> bool {
    child(node, name).is_some_and(crate::xml::on_off)
}

pub fn merge_field_name(instruction: &str) -> Option<String> {
    let rest = instruction.trim_start();
    let keyword = rest.get(..10)?;
    if !keyword.eq_ignore_ascii_case("MERGEFIELD") {
        return None;
    }
    let rest = &rest[10..];
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim_start();
    let raw = if let Some(quoted) = rest.strip_prefix('"') {
        &quoted[..quoted.find('"')?]
    } else {
        rest.split(char::is_whitespace).next()?
    };
    let name = raw
        .replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!name.is_empty()).then_some(name)
}

pub struct Converter<'p, 'b> {
    package: &'p mut Package<'b>,
    styles: Styles,
    numbering: Numbering,
    theme: Theme,
    catalog: &'p FieldCatalog,
    markers: &'p FieldMarkers,
    pub notices: Vec<Notice>,
    pub rels: Relationships,
    fields: Vec<OpenField>,
    counters: HashMap<String, [Option<i64>; 9]>,
    seen_nums: HashSet<(String, u8)>,
    table_key: String,
    table_style: Rc<Resolved>,
    para_cache: HashMap<(String, String), (Rc<ParaProps>, Option<String>)>,
    base_cache: HashMap<(String, Option<String>), (usize, Rc<RunProps>)>,
    run_cache: HashMap<(usize, String), Rc<RunStyle>>,
    styles_interned: HashMap<String, Style>,
    css_cache: HashMap<(usize, u64, u8), Option<Style>>,
    pub sections: Vec<Section>,
    section_index: usize,
}

impl<'p, 'b> Converter<'p, 'b> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        package: &'p mut Package<'b>,
        styles: Styles,
        numbering: Numbering,
        theme: Theme,
        catalog: &'p FieldCatalog,
        markers: &'p FieldMarkers,
        rels: Relationships,
        sections: Vec<Section>,
    ) -> Converter<'p, 'b> {
        let table_style = styles.none();
        Converter {
            package,
            styles,
            numbering,
            theme,
            catalog,
            markers,
            notices: Vec::new(),
            rels,
            fields: Vec::new(),
            counters: HashMap::new(),
            seen_nums: HashSet::new(),
            table_key: String::new(),
            table_style,
            para_cache: HashMap::new(),
            base_cache: HashMap::new(),
            run_cache: HashMap::new(),
            styles_interned: HashMap::new(),
            css_cache: HashMap::new(),
            sections,
            section_index: 0,
        }
    }

    pub fn read_text(&mut self, target: &str) -> Option<String> {
        self.package.read_text(target)
    }

    pub fn relationships(&mut self, target: &str) -> Relationships {
        self.package.relationships(target)
    }

    pub fn notice(&mut self, notice: Notice) {
        if !self.notices.contains(&notice) {
            self.notices.push(notice);
        }
    }

    fn intern(&mut self, css: String) -> Option<Style> {
        if css.is_empty() {
            return None;
        }
        if let Some(style) = self.styles_interned.get(&css) {
            return Some(style.clone());
        }
        let style = Style::new(&css);
        self.styles_interned.insert(css, style.clone());
        Some(style)
    }

    fn run_css(&mut self, format: &RunFormat, line: Line) -> Option<Style> {
        let rule = match line.rule {
            crate::props::LineRule::Auto => 0,
            crate::props::LineRule::Exact => 1,
            crate::props::LineRule::AtLeast => 2,
        };
        let key = (
            std::ptr::from_ref(format) as usize,
            line.value.to_bits(),
            rule,
        );
        if let Some(found) = self.css_cache.get(&key) {
            return found.clone();
        }
        let style = self.intern(format.css(line));
        self.css_cache.insert(key, style.clone());
        style
    }

    pub fn body(&mut self, body: Node) -> Vec<Block> {
        let items = self.container(body);
        self.finish(items, true)
    }

    pub fn part(&mut self, root: Node) -> Vec<Block> {
        let saved = std::mem::take(&mut self.fields);
        let items = self.container(root);
        self.fields = saved;
        self.finish(items, false)
    }

    fn container(&mut self, node: Node) -> Vec<Item> {
        let mut items = Vec::new();
        self.container_into(node, &mut items);
        items
    }

    fn container_into(&mut self, node: Node, items: &mut Vec<Item>) {
        for element in node.children().filter(Node::is_element) {
            if is_ns(element, MC, "AlternateContent") {
                if let Some(branch) = alternate_branch(element) {
                    self.container_into(branch, items);
                }
                continue;
            }
            match w_name(element) {
                Some("p") => self.paragraph(element, items),
                Some("tbl") => {
                    let table = self.table(element);
                    items.push(Item::Table(table));
                }
                Some("sdt") => {
                    if let Some(content) = child(element, "sdtContent") {
                        self.container_into(content, items);
                    }
                }
                Some("customXml" | "ins" | "moveTo" | "sdtContent") => {
                    self.container_into(element, items)
                }
                Some("altChunk") => self.notice(Notice::EmbeddedContentOmitted),
                _ => {}
            }
        }
    }

    fn para_props(&mut self, ppr: Option<Node>) -> (Rc<ParaProps>, Option<String>) {
        let source = ppr
            .map(|node| node.document().input_text()[node.range()].to_owned())
            .unwrap_or_default();
        let key = (self.table_key.clone(), source);
        if let Some(found) = self.para_cache.get(&key) {
            return found.clone();
        }
        let direct = ppr.map(ParaProps::parse).unwrap_or_default();
        let mut props = self.styles.default_ppr.clone();
        let style = self.styles.paragraph(direct.style.as_deref());
        if self.styles.is_default_paragraph(direct.style.as_deref()) {
            props.overlay(&style.ppr);
            props.overlay(&self.table_style.ppr);
        } else {
            props.overlay(&self.table_style.ppr);
            props.overlay(&style.ppr);
        }
        let numbering = direct
            .numbering
            .clone()
            .or_else(|| style.ppr.numbering.clone());
        if let Some((Some(id), level)) = &numbering
            && let Some(level) = self.numbering.level(id, level.unwrap_or(0))
        {
            props.overlay(&level.ppr);
        }
        props.overlay(&direct);
        props.numbering = numbering;
        let style_id = direct.style.clone();
        let value = (Rc::new(props), style_id);
        self.para_cache.insert(key, value.clone());
        value
    }

    fn run_base(&mut self, style_id: Option<&str>) -> (usize, Rc<RunProps>) {
        let key = (self.table_key.clone(), style_id.map(str::to_owned));
        if let Some(found) = self.base_cache.get(&key) {
            return found.clone();
        }
        let mut props = self.styles.default_rpr.clone();
        let style = self.styles.paragraph(style_id);
        if self.styles.is_default_paragraph(style_id) {
            props.toggle_overlay(&style.rpr);
            props.toggle_overlay(&self.table_style.rpr);
        } else {
            props.toggle_overlay(&self.table_style.rpr);
            props.toggle_overlay(&style.rpr);
        }
        let value = (self.base_cache.len(), Rc::new(props));
        self.base_cache.insert(key, value.clone());
        value
    }

    fn run_style(&mut self, base: &(usize, Rc<RunProps>), rpr: Option<Node>) -> Rc<RunStyle> {
        let source = rpr
            .map(|node| node.document().input_text()[node.range()].to_owned())
            .unwrap_or_default();
        let key = (base.0, source);
        if let Some(found) = self.run_cache.get(&key) {
            return found.clone();
        }
        let direct = rpr.map(RunProps::parse).unwrap_or_default();
        let mut props = (*base.1).clone();
        props.toggle_overlay(&self.styles.character(direct.style.as_deref()).rpr);
        props.overlay(&direct);
        let style = Rc::new(RunStyle {
            normal: RunFormat::resolve(&props, false, &self.theme),
            complex: RunFormat::resolve(&props, true, &self.theme),
            forced: props.rtl.unwrap_or(false) || props.cs.unwrap_or(false),
            hint_cs: props.fonts.hint_cs.unwrap_or(false),
            hidden: props.vanish.unwrap_or(false),
        });
        self.run_cache.insert(key, style.clone());
        style
    }

    fn paragraph(&mut self, p: Node, items: &mut Vec<Item>) {
        let ppr = child(p, "pPr");
        let (props, style_id) = self.para_props(ppr);
        let base_style = style_id.clone();
        let base = self.run_base(base_style.as_deref());
        let mark = self.run_style(&base, ppr.and_then(|ppr| child(ppr, "rPr")));
        let mut atoms = Vec::new();
        let mut deferred = Vec::new();
        self.inline_children(p, &base, &mut atoms, &mut deferred);
        let pieces = self.resolve_fields(atoms);
        let pieces = self.join_tokens(pieces);
        let layout = self.layout(&props, &mark, style_id);
        let list = self.list_info(&props);
        let mut segments: Vec<Vec<Piece>> = vec![Vec::new()];
        for piece in pieces {
            if matches!(piece, Piece::PageBreak) {
                segments.push(Vec::new());
            } else if let Some(last) = segments.last_mut() {
                last.push(piece);
            }
        }
        if props.page_break_before.unwrap_or(false) {
            items.push(Item::PageBreak);
        }
        let all_empty = segments.iter().all(Vec::is_empty);
        let mut list = list;
        for (index, segment) in segments.into_iter().enumerate() {
            if index > 0 {
                items.push(Item::PageBreak);
            }
            if segment.is_empty() && !(all_empty && index == 0) {
                continue;
            }
            let children = self.inlines(&segment, layout.line);
            let paragraph = self.paragraph_node(children, &layout, &mark);
            items.push(Item::Para(Box::new(ParaItem {
                paragraph,
                layout: layout.clone(),
                list: list.take(),
            })));
        }
        for content in deferred {
            self.container_into(content, items);
        }
        if ppr.and_then(|ppr| child(ppr, "sectPr")).is_some() {
            items.push(Item::Section(self.section_index));
            self.section_index += 1;
        }
    }

    fn paragraph_node(
        &mut self,
        children: Vec<Inline>,
        layout: &Layout,
        mark: &Rc<RunStyle>,
    ) -> Paragraph {
        let first_text = children.iter().find_map(|inline| match inline {
            Inline::Text(text) => Some((
                text.style.clone(),
                (text.format.0 != 0).then_some(text.format),
            )),
            _ => None,
        });
        let (text_style, text_format) = match first_text {
            Some(found) => found,
            None if children.is_empty() => {
                let format = mark.format(mark.forced || layout.rtl);
                let css = format.css(layout.line);
                let text_format = format.text_format();
                (
                    self.intern(css),
                    (text_format.0 != 0).then_some(text_format),
                )
            }
            None => (None, None),
        };
        Paragraph {
            props: ElementProps {
                direction: Some(if layout.rtl {
                    Direction::Rtl
                } else {
                    Direction::Ltr
                }),
                align: Some(layout.align),
                text_style,
                text_format,
                ..ElementProps::default()
            },
            children,
        }
    }

    fn layout(&mut self, props: &ParaProps, mark: &RunStyle, style: Option<String>) -> Layout {
        let rtl = props.bidi.unwrap_or(false);
        let align = match props.jc {
            Some(Jc::End) => Align::End,
            Some(Jc::Center) => Align::Center,
            Some(Jc::Justify) => Align::Justify,
            Some(Jc::Start) | None => Align::Start,
        };
        let auto = 280.0;
        Layout {
            rtl,
            align,
            start: props.ind_start.unwrap_or(0.0),
            end: props.ind_end.unwrap_or(0.0),
            first: props.ind_first.unwrap_or(0.0),
            before: if props.before_auto.unwrap_or(false) {
                auto
            } else {
                props.before.unwrap_or(0.0)
            },
            after: if props.after_auto.unwrap_or(false) {
                auto
            } else {
                props.after.unwrap_or(0.0)
            },
            contextual: props.contextual_spacing.unwrap_or(false),
            style,
            line: Line::from(props.line),
            font: mark.format(mark.forced || rtl).clone(),
            shading: props.shading.flatten().filter(|color| !color.is_white()),
            borders: props.borders,
        }
    }

    fn list_info(&mut self, props: &ParaProps) -> Option<ListInfo> {
        let (Some(id), level) = props.numbering.clone()? else {
            return None;
        };
        let level_index = level.unwrap_or(0).min(8);
        let level = self.numbering.level(&id, level_index)?;
        if level.format == "none" {
            return None;
        }
        let group = self
            .numbering
            .abstract_of(&id)
            .unwrap_or_default()
            .to_owned();
        let restart = !self.seen_nums.contains(&(id.clone(), level_index))
            && self.numbering.has_start_override(&id, level_index);
        self.seen_nums.insert((id.clone(), level_index));
        let counters = self.counters.entry(group.clone()).or_insert([None; 9]);
        let index = usize::from(level_index);
        let value = match counters[index] {
            Some(current) if !restart => current + 1,
            _ => level.start,
        };
        counters[index] = Some(value);
        for deeper in counters.iter_mut().skip(index + 1) {
            *deeper = None;
        }
        Some(ListInfo {
            group,
            level: level_index,
            value,
            bullet: level.format == "bullet",
        })
    }

    fn inline_children<'a, 'i>(
        &mut self,
        node: Node<'a, 'i>,
        base: &(usize, Rc<RunProps>),
        atoms: &mut Vec<Atom>,
        deferred: &mut Vec<Node<'a, 'i>>,
    ) {
        for element in node.children().filter(Node::is_element) {
            if is_ns(element, MC, "AlternateContent") {
                if let Some(branch) = alternate_branch(element) {
                    self.inline_children(branch, base, atoms, deferred);
                }
                continue;
            }
            if element.tag_name().namespace() == Some(M) {
                if matches!(element.tag_name().name(), "oMath" | "oMathPara") {
                    self.notice(Notice::EquationsNeedChecking);
                    let text = text_of(element);
                    if !text.trim().is_empty() {
                        let style = self.run_style(base, None);
                        atoms.push(Atom::Text(text, style));
                    }
                }
                continue;
            }
            match w_name(element) {
                Some("r") => self.run(element, base, atoms, deferred),
                Some("hyperlink") => {
                    let url = r_attr(element, "id")
                        .and_then(|id| self.rels.get(id))
                        .filter(|relationship| relationship.external)
                        .map(|relationship| relationship.target.clone())
                        .or_else(|| attr(element, "anchor").map(|anchor| format!("#{anchor}")));
                    atoms.push(Atom::LinkStart(url));
                    self.inline_children(element, base, atoms, deferred);
                    atoms.push(Atom::LinkEnd);
                }
                Some("fldSimple") => {
                    let instruction = attr(element, "instr").unwrap_or_default();
                    match merge_field_name(instruction) {
                        Some(name) => {
                            let first = element
                                .descendants()
                                .find(|node| is_w(*node, "r"))
                                .and_then(|run| child(run, "rPr"));
                            let style = self.run_style(base, first);
                            atoms.push(Atom::Merge(name, style));
                        }
                        None => self.inline_children(element, base, atoms, deferred),
                    }
                }
                Some(
                    "ins" | "moveTo" | "smartTag" | "customXml" | "dir" | "bdo" | "sdtContent"
                    | "rubyBase",
                ) => self.inline_children(element, base, atoms, deferred),
                Some("sdt") => {
                    if let Some(content) = child(element, "sdtContent") {
                        self.inline_children(content, base, atoms, deferred);
                    }
                }
                _ => {}
            }
        }
    }

    fn run<'a, 'i>(
        &mut self,
        run: Node<'a, 'i>,
        base: &(usize, Rc<RunProps>),
        atoms: &mut Vec<Atom>,
        deferred: &mut Vec<Node<'a, 'i>>,
    ) {
        let style = self.run_style(base, child(run, "rPr"));
        self.run_content(run, base, &style, atoms, deferred);
    }

    fn run_content<'a, 'i>(
        &mut self,
        node: Node<'a, 'i>,
        base: &(usize, Rc<RunProps>),
        style: &Rc<RunStyle>,
        atoms: &mut Vec<Atom>,
        deferred: &mut Vec<Node<'a, 'i>>,
    ) {
        let hidden = style.hidden;
        let text = |atoms: &mut Vec<Atom>, text: String| {
            if !hidden && !text.is_empty() {
                atoms.push(Atom::Text(text, style.clone()));
            }
        };
        for element in node.children().filter(Node::is_element) {
            if is_ns(element, MC, "AlternateContent") {
                if let Some(branch) = alternate_branch(element) {
                    self.run_content(branch, base, style, atoms, deferred);
                }
                continue;
            }
            match w_name(element) {
                Some("t") => text(atoms, element.text().unwrap_or_default().to_owned()),
                Some("tab" | "ptab") => text(atoms, "\u{2003}".to_owned()),
                Some("noBreakHyphen") => text(atoms, "\u{2011}".to_owned()),
                Some("sym") => {
                    let code = attr(element, "char")
                        .and_then(|code| u32::from_str_radix(code, 16).ok())
                        .map(|code| if code >= 0xF000 { code - 0xF000 } else { code })
                        .and_then(char::from_u32);
                    if let Some(code) = code {
                        text(atoms, code.to_string());
                    }
                }
                Some("br") => {
                    if hidden {
                        continue;
                    }
                    if attr(element, "type") == Some("page") {
                        atoms.push(Atom::PageBreak);
                    } else {
                        atoms.push(Atom::LineBreak);
                    }
                }
                Some("cr") if !hidden => atoms.push(Atom::LineBreak),
                Some("fldChar") => match attr(element, "fldCharType") {
                    Some("begin") => atoms.push(Atom::Begin(style.clone())),
                    Some("separate") => atoms.push(Atom::Separate),
                    Some("end") => atoms.push(Atom::End),
                    _ => {}
                },
                Some("instrText") => {
                    atoms.push(Atom::Instr(element.text().unwrap_or_default().to_owned()))
                }
                Some("drawing") => {
                    if !hidden {
                        self.drawing(element, deferred);
                    }
                }
                Some("pict" | "object") => {
                    if !hidden {
                        self.vml(element, deferred);
                    }
                }
                Some("footnoteReference" | "endnoteReference") => {
                    self.notice(Notice::FootnotesOmitted)
                }
                Some("ruby") => {
                    if let Some(ruby_base) = child(element, "rubyBase") {
                        self.inline_children(ruby_base, base, atoms, deferred);
                    }
                }
                _ => {}
            }
        }
    }

    fn text_boxes<'a, 'i>(&self, node: Node<'a, 'i>, deferred: &mut Vec<Node<'a, 'i>>) -> bool {
        let mut found = false;
        for content in node.descendants().filter(|node| is_w(*node, "txbxContent")) {
            let nested = content
                .ancestors()
                .skip(1)
                .take_while(|ancestor| *ancestor != node)
                .any(|ancestor| is_w(ancestor, "txbxContent"));
            if !nested {
                deferred.push(content);
                found = true;
            }
        }
        found
    }

    fn drawing<'a, 'i>(&mut self, drawing: Node<'a, 'i>, deferred: &mut Vec<Node<'a, 'i>>) {
        if drawing.children().any(|node| is_ns(node, WP, "anchor")) {
            self.notice(Notice::FloatingFlattened);
        }
        let has_text = self.text_boxes(drawing, deferred);
        let picture = drawing
            .descendants()
            .any(|node| node.is_element() && node.tag_name().name() == "blip");
        if picture || !has_text {
            self.notice(Notice::DrawingsOmitted);
        }
    }

    fn vml<'a, 'i>(&mut self, node: Node<'a, 'i>, deferred: &mut Vec<Node<'a, 'i>>) {
        let floating = node.descendants().any(|shape| {
            shape.tag_name().namespace() == Some(V)
                && plain_attr(shape, "style")
                    .is_some_and(|style| style.contains("position:absolute"))
        });
        if floating {
            self.notice(Notice::FloatingFlattened);
        }
        let has_text = self.text_boxes(node, deferred);
        let picture = node.descendants().any(|item| is_ns(item, V, "imagedata"));
        if !picture && is_w(node, "object") {
            self.notice(Notice::EmbeddedContentOmitted);
        } else if picture || !has_text {
            self.notice(Notice::DrawingsOmitted);
        }
    }

    fn resolve_fields(&mut self, atoms: Vec<Atom>) -> Vec<Piece> {
        let mut out: Vec<Piece> = Vec::new();
        let visible = |fields: &Vec<OpenField>| fields.iter().all(|field| field.in_result);
        for atom in atoms {
            match atom {
                Atom::Begin(style) => self.fields.push(OpenField {
                    instruction: String::new(),
                    in_result: false,
                    start: out.len(),
                    result_style: None,
                    begin_style: style,
                    cross: false,
                }),
                Atom::Separate => {
                    if let Some(field) = self.fields.last_mut() {
                        field.in_result = true;
                    }
                }
                Atom::End => {
                    let Some(field) = self.fields.pop() else {
                        continue;
                    };
                    let parent_ok = self.fields.last().is_none_or(|parent| parent.in_result);
                    if let Some(name) = merge_field_name(&field.instruction)
                        && !field.cross
                        && parent_ok
                    {
                        out.truncate(field.start);
                        let style = field.result_style.unwrap_or(field.begin_style);
                        let complex = style.forced
                            || style.hint_cs
                            || name.chars().any(crate::format::is_complex);
                        let reference = self.catalog.reference_for(&name);
                        out.push(Piece::Field(reference, style, complex));
                    }
                }
                Atom::Instr(text) => {
                    if let Some(field) = self.fields.last_mut()
                        && !field.in_result
                    {
                        field.instruction.push_str(&text);
                    }
                }
                Atom::Merge(name, style) => {
                    if visible(&self.fields) {
                        let complex = style.forced
                            || style.hint_cs
                            || name.chars().any(crate::format::is_complex);
                        let reference = self.catalog.reference_for(&name);
                        out.push(Piece::Field(reference, style, complex));
                    }
                }
                Atom::Text(text, style) => {
                    if visible(&self.fields) {
                        if let Some(field) = self.fields.last_mut()
                            && field.result_style.is_none()
                        {
                            field.result_style = Some(style.clone());
                        }
                        out.push(Piece::Text(text, style));
                    }
                }
                Atom::LineBreak => {
                    if visible(&self.fields) {
                        out.push(Piece::LineBreak);
                    }
                }
                Atom::PageBreak => {
                    if visible(&self.fields) {
                        out.push(Piece::PageBreak);
                    }
                }
                Atom::LinkStart(url) => out.push(Piece::LinkStart(url)),
                Atom::LinkEnd => out.push(Piece::LinkEnd),
            }
        }
        self.fields.retain(|field| field.in_result);
        for field in &mut self.fields {
            field.cross = true;
            field.start = 0;
        }
        out
    }

    fn join_tokens(&self, pieces: Vec<Piece>) -> Vec<Piece> {
        let open = self.markers.open();
        if !pieces
            .iter()
            .any(|piece| matches!(piece, Piece::Text(text, _) if text.contains(open)))
        {
            return pieces;
        }
        let mut out = Vec::with_capacity(pieces.len());
        let mut group: Vec<(String, Rc<RunStyle>)> = Vec::new();
        for piece in pieces {
            match piece {
                Piece::Text(text, style) => group.push((text, style)),
                other => {
                    self.flush_tokens(std::mem::take(&mut group), &mut out);
                    out.push(other);
                }
            }
        }
        self.flush_tokens(group, &mut out);
        out
    }

    fn flush_tokens(&self, group: Vec<(String, Rc<RunStyle>)>, out: &mut Vec<Piece>) {
        if group.is_empty() {
            return;
        }
        let mut content = String::new();
        let mut starts = Vec::with_capacity(group.len());
        for (text, _) in &group {
            starts.push(content.len());
            content.push_str(text);
        }
        let mut matches = Vec::new();
        let mut from = 0;
        while let Some(found) = self.catalog.find_token(&content, self.markers, from) {
            from = found.range.end;
            matches.push(found);
        }
        if matches.is_empty() {
            out.extend(
                group
                    .into_iter()
                    .map(|(text, style)| Piece::Text(text, style)),
            );
            return;
        }
        let piece_at = |offset: usize| {
            starts
                .iter()
                .rposition(|start| *start <= offset)
                .unwrap_or(0)
        };
        let mut cursor = 0;
        let emit = |out: &mut Vec<Piece>, from: usize, to: usize| {
            let mut position = from;
            while position < to {
                let index = piece_at(position);
                let end = starts
                    .get(index + 1)
                    .copied()
                    .unwrap_or(content.len())
                    .min(to);
                if end > position {
                    out.push(Piece::Text(
                        content[position..end].to_owned(),
                        group[index].1.clone(),
                    ));
                }
                position = end.max(position + 1);
            }
        };
        for found in matches {
            emit(out, cursor, found.range.start);
            let style = group[piece_at(found.range.start)].1.clone();
            let name = &content[found.range.clone()];
            let complex =
                style.forced || style.hint_cs || name.chars().any(crate::format::is_complex);
            out.push(Piece::Field(found.reference, style, complex));
            cursor = found.range.end;
        }
        emit(out, cursor, content.len());
    }

    fn inlines(&mut self, pieces: &[Piece], line: Line) -> Vec<Inline> {
        let mut stack: Vec<(Option<String>, Vec<Inline>)> = vec![(None, Vec::new())];
        for piece in pieces {
            match piece {
                Piece::Text(text, style) => {
                    for (segment, complex) in segments(text, style) {
                        let format = style.format(complex);
                        let text_format = format.text_format();
                        let style = self.run_css(format, line);
                        let Some((_, current)) = stack.last_mut() else {
                            continue;
                        };
                        push_text(current, segment, style, text_format);
                    }
                }
                Piece::Field(reference, style, complex) => {
                    let imported_style = self.run_css(style.format(*complex), line);
                    if let Some((_, current)) = stack.last_mut() {
                        let mut field = Field::new(reference.clone());
                        field.imported_style = imported_style;
                        current.push(Inline::Field(field));
                    }
                }
                Piece::LineBreak => {
                    if let Some((_, current)) = stack.last_mut() {
                        current.push(Inline::LineBreak(Leaf::default()));
                    }
                }
                Piece::LinkStart(url) => stack.push((url.clone(), Vec::new())),
                Piece::LinkEnd => close_link(&mut stack),
                Piece::PageBreak => {}
            }
        }
        while stack.len() > 1 {
            close_link(&mut stack);
        }
        stack.pop().map(|(_, inlines)| inlines).unwrap_or_default()
    }

    fn table(&mut self, node: Node) -> Table {
        let direct = child(node, "tblPr")
            .map(TableProps::parse)
            .unwrap_or_default();
        let style = self.styles.table(direct.style.as_deref());
        let mut props = style.tblpr.clone();
        props.overlay(&direct);
        if props.floating {
            self.notice(Notice::FloatingFlattened);
        }
        let rtl = props.bidi.unwrap_or(false);
        let grid: Vec<f64> = child(node, "tblGrid")
            .map(|grid| {
                grid.children()
                    .filter(|column| is_w(*column, "gridCol"))
                    .map(|column| twips(column, "w").unwrap_or(0.0))
                    .collect()
            })
            .unwrap_or_default();
        let saved_key = std::mem::replace(
            &mut self.table_key,
            direct
                .style
                .clone()
                .unwrap_or_else(|| "\u{0}default".to_owned()),
        );
        let saved_style = std::mem::replace(&mut self.table_style, style.clone());
        let saved_fields = std::mem::take(&mut self.fields);

        struct CellMeta {
            column: usize,
            span: usize,
            row_span: usize,
            props: CellProps,
            blocks: Vec<Block>,
            placeholder: bool,
        }
        struct RowMeta {
            props: RowProps,
            exception: Option<TableProps>,
            cells: Vec<CellMeta>,
        }
        let mut rows: Vec<RowMeta> = Vec::new();
        let mut merges: HashMap<usize, (usize, usize)> = HashMap::new();
        for row in table_rows(node) {
            let row_props = RowProps::parse(child(row, "trPr"));
            let exception = child(row, "tblPrEx").map(TableProps::parse);
            let mut column = row_props.grid_before as usize;
            let mut cells: Vec<CellMeta> = Vec::new();
            let row_index = rows.len();
            let placeholder = |column: usize, span: u32, width: Option<Width>| CellMeta {
                column,
                span: span as usize,
                row_span: 1,
                props: CellProps {
                    width,
                    span,
                    ..CellProps::default()
                },
                blocks: vec![Block::Paragraph(Paragraph {
                    props: ElementProps {
                        direction: Some(if rtl { Direction::Rtl } else { Direction::Ltr }),
                        align: Some(Align::Start),
                        ..ElementProps::default()
                    },
                    children: Vec::new(),
                })],
                placeholder: true,
            };
            if row_props.grid_before > 0 {
                cells.push(placeholder(
                    0,
                    row_props.grid_before,
                    row_props.before_width,
                ));
            }
            for cell in row_cells(row) {
                let cell_props = CellProps::parse(child(cell, "tcPr"));
                let span = cell_props.span as usize;
                if cell_props.h_merge == Some(VMerge::Continue)
                    && let Some(last) = cells.last_mut()
                {
                    last.span += span;
                    column += span;
                    continue;
                }
                if cell_props.v_merge == Some(VMerge::Continue)
                    && let Some(&(origin_row, origin_cell)) = merges.get(&column)
                    && let Some(origin) = rows
                        .get_mut(origin_row)
                        .and_then(|row| row.cells.get_mut(origin_cell))
                    && origin.span == span
                {
                    origin.row_span += 1;
                    column += span;
                    continue;
                }
                let items = self.container(cell);
                let mut blocks = self.finish(items, false);
                if blocks.is_empty() {
                    blocks.push(Block::Paragraph(Paragraph {
                        props: ElementProps {
                            direction: Some(if rtl { Direction::Rtl } else { Direction::Ltr }),
                            align: Some(Align::Start),
                            ..ElementProps::default()
                        },
                        children: Vec::new(),
                    }));
                }
                for covered in column..column + span {
                    merges.remove(&covered);
                }
                if cell_props.v_merge == Some(VMerge::Restart) {
                    merges.insert(column, (row_index, cells.len()));
                }
                cells.push(CellMeta {
                    column,
                    span,
                    row_span: 1,
                    props: cell_props,
                    blocks,
                    placeholder: false,
                });
                column += span;
            }
            if row_props.grid_after > 0 {
                cells.push(placeholder(
                    column,
                    row_props.grid_after,
                    row_props.after_width,
                ));
            }
            rows.push(RowMeta {
                props: row_props,
                exception,
                cells,
            });
        }
        self.table_key = saved_key;
        self.table_style = saved_style;
        self.fields = saved_fields;

        let columns = rows
            .iter()
            .flat_map(|row| row.cells.iter().map(|cell| cell.column + cell.span))
            .chain(std::iter::once(grid.len()))
            .max()
            .unwrap_or(0);
        let last_row = rows.len().saturating_sub(1);
        let direction = Some(if rtl { Direction::Rtl } else { Direction::Ltr });
        let mut table_rows = Vec::with_capacity(rows.len());
        for (row_index, row) in rows.into_iter().enumerate() {
            let mut table_props = props.clone();
            if let Some(exception) = &row.exception {
                table_props.borders.overlay(&exception.borders);
                table_props.cell_margins.overlay(&exception.cell_margins);
            }
            let mut cells = Vec::with_capacity(row.cells.len());
            for cell in row.cells {
                if cell.placeholder {
                    let mut css = Css::default();
                    match cell.props.width {
                        Some(Width::Twips(width)) => css.push("width", &pt(width / 20.0)),
                        Some(Width::Percent(percent)) => {
                            css.push("width", &format!("{}%", crate::format::number(percent)))
                        }
                        None => {}
                    }
                    css.push("padding", "0px");
                    let imported_style = self.intern(css.finish());
                    cells.push(TableCell {
                        props: ElementProps {
                            direction,
                            imported_style,
                            ..ElementProps::default()
                        },
                        col_span: (cell.span > 1).then_some(cell.span as u32),
                        blocks: cell.blocks,
                        ..TableCell::default()
                    });
                    continue;
                }
                let mut cell_borders = style.tcpr.borders;
                cell_borders.overlay(&cell.props.borders);
                let table_borders = &table_props.borders;
                let pick = |own: Option<Option<Border>>,
                            outer: bool,
                            edge: Option<Option<Border>>,
                            inside: Option<Option<Border>>| {
                    own.or(if outer { edge } else { inside }).flatten()
                };
                let resolved = Sides {
                    top: Some(pick(
                        cell_borders.top,
                        row_index == 0,
                        table_borders.top,
                        table_borders.inside_h,
                    )),
                    bottom: Some(pick(
                        cell_borders.bottom,
                        row_index + cell.row_span > last_row,
                        table_borders.bottom,
                        table_borders.inside_h,
                    )),
                    start: Some(pick(
                        cell_borders.start,
                        cell.column == 0,
                        table_borders.start,
                        table_borders.inside_v,
                    )),
                    end: Some(pick(
                        cell_borders.end,
                        cell.column + cell.span >= columns,
                        table_borders.end,
                        table_borders.inside_v,
                    )),
                    inside_h: None,
                    inside_v: None,
                };
                let mut margins = Sides {
                    top: Some(0.0),
                    bottom: Some(0.0),
                    start: Some(108.0),
                    end: Some(108.0),
                    inside_h: None,
                    inside_v: None,
                };
                margins.overlay(&style.tcpr.margins);
                margins.overlay(&table_props.cell_margins);
                margins.overlay(&cell.props.margins);
                let shading = cell
                    .props
                    .shading
                    .or(style.tcpr.shading)
                    .or(table_props.shading)
                    .flatten();
                let v_align = cell.props.v_align.or(style.tcpr.v_align);
                let mut css = Css::default();
                match cell.props.width {
                    Some(Width::Twips(width)) => css.push("width", &pt(width / 20.0)),
                    Some(Width::Percent(percent)) => {
                        css.push("width", &format!("{}%", crate::format::number(percent)))
                    }
                    None => {}
                }
                let padding: Vec<String> = physical(&margins, rtl)
                    .iter()
                    .map(|side| px(side.unwrap_or(0.0) / 15.0))
                    .collect();
                css.push("padding", &four(&padding));
                css.push("vertical-align", v_align.unwrap_or("top"));
                let [top, right, bottom, left] = physical(&resolved, rtl);
                css.borders([
                    top.flatten(),
                    right.flatten(),
                    bottom.flatten(),
                    left.flatten(),
                ]);
                if let Some(shading) = shading {
                    css.push("background-color", &shading.css());
                }
                let imported_style = self.intern(css.finish());
                cells.push(TableCell {
                    props: ElementProps {
                        direction,
                        imported_style,
                        ..ElementProps::default()
                    },
                    col_span: (cell.span > 1).then_some(cell.span as u32),
                    row_span: (cell.row_span > 1).then_some(cell.row_span as u32),
                    background_color: shading.map(Rgb::css),
                    vertical_align: v_align.filter(|align| *align != "top").map(str::to_owned),
                    blocks: cell.blocks,
                    ..TableCell::default()
                });
            }
            let mut row_css = Css::default();
            if let Some((height, _)) = row.props.height {
                row_css.push("height", &pt(height / 20.0));
            }
            let imported_style = self.intern(row_css.finish());
            table_rows.push(TableRow {
                props: ElementProps {
                    direction,
                    imported_style,
                    ..ElementProps::default()
                },
                cells,
                ..TableRow::default()
            });
        }
        let mut css = Css::default();
        if let Some(Some(width)) = props.width {
            match width {
                Width::Twips(width) => css.push("width", &pt(width / 20.0)),
                Width::Percent(percent) => {
                    css.push("width", &format!("{}%", crate::format::number(percent)))
                }
            }
        }
        let indent = px(props.indent.unwrap_or(0.0) / 15.0);
        let zero = "0px".to_owned();
        let margin = match props.jc {
            Some(Jc::Center) => vec![
                zero.clone(),
                "auto".to_owned(),
                zero.clone(),
                "auto".to_owned(),
            ],
            _ if rtl => vec![zero.clone(), indent, zero.clone(), zero.clone()],
            _ => vec![zero.clone(), zero.clone(), zero.clone(), indent],
        };
        css.push("margin", &four(&margin));
        css.push("border-collapse", "collapse");
        let imported_style = self.intern(css.finish());
        Table {
            props: ElementProps {
                direction,
                imported_style,
                ..ElementProps::default()
            },
            col_widths: (!grid.is_empty())
                .then(|| grid.iter().map(|width| round2(width / 15.0)).collect()),
            rows: table_rows,
            ..Table::default()
        }
    }

    fn paragraph_css(&self, layout: &Layout) -> String {
        let mut css = Css::default();
        if let Some(family) = &layout.font.family {
            css.push("font-family", &crate::format::family_css(family));
        }
        css.push("font-size", &px(layout.font.size_px));
        css.push(
            "line-height",
            &px(round2(
                layout
                    .line
                    .height(layout.font.family.as_deref(), layout.font.size_px),
            )),
        );
        css.push("text-align", align_css(layout.align));
        css.push("text-indent", &px(layout.first / 15.0));
        let (left, right) = if layout.rtl {
            (layout.end, layout.start)
        } else {
            (layout.start, layout.end)
        };
        let margins = [layout.before, right, layout.after, left].map(|twips| px(twips / 15.0));
        css.push("margin", &four(&margins));
        if let Some(shading) = layout.shading {
            css.push("background-color", &shading.css());
        }
        let [top, right, bottom, left] = physical(&layout.borders, layout.rtl);
        css.borders([
            top.flatten(),
            right.flatten(),
            bottom.flatten(),
            left.flatten(),
        ]);
        css.finish()
    }

    fn finish(&mut self, mut items: Vec<Item>, root: bool) -> Vec<Block> {
        for index in 1..items.len() {
            let (head, tail) = items.split_at_mut(index);
            if let (Item::Para(previous), Item::Para(next)) = (&mut head[index - 1], &mut tail[0])
                && previous.layout.style == next.layout.style
            {
                if previous.layout.contextual {
                    previous.layout.after = 0.0;
                }
                if next.layout.contextual {
                    next.layout.before = 0.0;
                }
            }
        }
        let mut blocks = Vec::with_capacity(items.len());
        let mut pending: Vec<ParaItem> = Vec::new();
        for item in items {
            let starts_new = match &item {
                Item::Para(para) => match (
                    &para.list,
                    pending.first().and_then(|first| first.list.as_ref()),
                ) {
                    (Some(list), Some(first)) => list.group != first.group && list.level == 0,
                    (Some(_), None) => false,
                    (None, _) => true,
                },
                _ => true,
            };
            if starts_new && !pending.is_empty() {
                let list = self.list(std::mem::take(&mut pending));
                blocks.push(Block::List(list));
            }
            match item {
                Item::Para(para) if para.list.is_some() => pending.push(*para),
                Item::Para(para) => {
                    let mut paragraph = para.paragraph;
                    paragraph.props.imported_style = self.intern(self.paragraph_css(&para.layout));
                    blocks.push(Block::Paragraph(paragraph));
                }
                Item::Table(table) => blocks.push(Block::Table(table)),
                Item::PageBreak => {
                    if root {
                        blocks.push(Block::PageBreak(Leaf::default()));
                    }
                }
                Item::Section(index) => {
                    let breaks = self
                        .sections
                        .get(index + 1)
                        .is_some_and(|section| section.breaks_page);
                    if root && breaks {
                        blocks.push(Block::PageBreak(Leaf::default()));
                    }
                }
            }
        }
        if !pending.is_empty() {
            let list = self.list(pending);
            blocks.push(Block::List(list));
        }
        blocks
    }

    fn list(&mut self, entries: Vec<ParaItem>) -> List {
        let mut stack: Vec<(u8, List)> = Vec::new();
        let new_list = |entry: &ParaItem, level: u8| {
            let info = entry.list.as_ref();
            let bullet = info.is_some_and(|info| info.bullet);
            (
                level,
                List {
                    props: ElementProps {
                        direction: entry.paragraph.props.direction,
                        ..ElementProps::default()
                    },
                    list_type: Some(if bullet {
                        ListType::Bullet
                    } else {
                        ListType::Number
                    }),
                    start: Some(info.map_or(1, |info| info.value)),
                    tag: Some(if bullet { ListTag::Ul } else { ListTag::Ol }),
                    items: Vec::new(),
                },
            )
        };
        fn close(stack: &mut Vec<(u8, List)>) {
            if let Some((_, list)) = stack.pop() {
                if let Some((_, parent)) = stack.last_mut() {
                    parent.items.push(ListItem {
                        props: ElementProps {
                            direction: list.props.direction,
                            ..ElementProps::default()
                        },
                        children: vec![ListItemChild::List(list)],
                        ..ListItem::default()
                    });
                } else {
                    stack.push((0, list));
                }
            }
        }
        for entry in entries {
            let level = entry.list.as_ref().map_or(0, |info| info.level);
            while stack.len() > 1 && stack.last().is_some_and(|(top, _)| *top > level) {
                close(&mut stack);
            }
            if stack.last().is_none_or(|(top, _)| *top < level) {
                stack.push(new_list(&entry, level));
            }
            let imported_style = self.intern(self.paragraph_css(&entry.layout));
            let props = entry.paragraph.props;
            if let Some((_, list)) = stack.last_mut() {
                list.items.push(ListItem {
                    props: ElementProps {
                        imported_style,
                        ..props
                    },
                    value: entry.list.as_ref().map(|info| info.value),
                    children: entry
                        .paragraph
                        .children
                        .into_iter()
                        .map(ListItemChild::Inline)
                        .collect(),
                    ..ListItem::default()
                });
            }
        }
        while stack.len() > 1 {
            close(&mut stack);
        }
        stack.pop().map(|(_, list)| list).unwrap_or_default()
    }
}

fn close_link(stack: &mut Vec<(Option<String>, Vec<Inline>)>) {
    if stack.len() < 2 {
        return;
    }
    let Some((url, children)) = stack.pop() else {
        return;
    };
    let Some((_, parent)) = stack.last_mut() else {
        return;
    };
    if children.is_empty() {
        return;
    }
    match url {
        Some(url) => parent.push(Inline::Link(Link {
            url: Some(url),
            children,
            ..Link::default()
        })),
        None => parent.extend(children),
    }
}

fn push_text(current: &mut Vec<Inline>, text: &str, style: Option<Style>, format: TextFormat) {
    if text.is_empty() {
        return;
    }
    if let Some(Inline::Text(previous)) = current.last_mut()
        && previous.style == style
        && previous.format == format
    {
        previous.text.push_str(text);
        return;
    }
    current.push(Inline::Text(Text {
        text: text.to_owned(),
        format,
        style,
        ..Text::default()
    }));
}

fn segments<'t>(text: &'t str, style: &RunStyle) -> Vec<(&'t str, bool)> {
    if style.forced {
        return vec![(text, true)];
    }
    let mut out: Vec<(&str, bool)> = Vec::new();
    let mut current: Option<bool> = None;
    let mut start = 0;
    for (index, c) in text.char_indices() {
        let class = match script(c) {
            Script::Complex => Some(true),
            Script::Latin => Some(false),
            Script::Neutral => None,
        };
        let Some(class) = class else { continue };
        match current {
            None => current = Some(class),
            Some(previous) if previous != class => {
                if index > start {
                    out.push((&text[start..index], previous));
                }
                start = index;
                current = Some(class);
            }
            _ => {}
        }
    }
    let class = current.unwrap_or(style.hint_cs);
    if start < text.len() {
        out.push((&text[start..], class));
    }
    out
}

fn table_rows<'a, 'i>(node: Node<'a, 'i>) -> Vec<Node<'a, 'i>> {
    let mut rows = Vec::new();
    for element in node.children().filter(Node::is_element) {
        match w_name(element) {
            Some("tr") => rows.push(element),
            Some("sdt") => {
                if let Some(content) = child(element, "sdtContent") {
                    rows.extend(table_rows(content));
                }
            }
            Some("customXml" | "ins" | "moveTo") => rows.extend(table_rows(element)),
            _ => {}
        }
    }
    rows
}

fn row_cells<'a, 'i>(node: Node<'a, 'i>) -> Vec<Node<'a, 'i>> {
    let mut cells = Vec::new();
    for element in node.children().filter(Node::is_element) {
        match w_name(element) {
            Some("tc") => cells.push(element),
            Some("sdt") => {
                if let Some(content) = child(element, "sdtContent") {
                    cells.extend(row_cells(content));
                }
            }
            Some("customXml" | "ins" | "moveTo") => cells.extend(row_cells(element)),
            _ => {}
        }
    }
    cells
}
