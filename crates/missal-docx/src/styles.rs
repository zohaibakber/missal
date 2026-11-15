use std::collections::HashMap;
use std::rc::Rc;

use roxmltree::Node;

use crate::props::{CellProps, ParaProps, RunProps, TableProps};
use crate::xml::{A, attr, child, children, is_ns, number, plain_attr, val};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Paragraph,
    Character,
    Table,
    Numbering,
}

#[derive(Debug, Clone)]
struct StyleDef {
    kind: Kind,
    based_on: Option<String>,
    ppr: ParaProps,
    rpr: RunProps,
    tblpr: TableProps,
    tcpr: CellProps,
}

#[derive(Debug, Clone, Default)]
pub struct Resolved {
    pub ppr: ParaProps,
    pub rpr: RunProps,
    pub tblpr: TableProps,
    pub tcpr: CellProps,
}

#[derive(Debug, Default)]
pub struct Styles {
    pub default_rpr: RunProps,
    pub default_ppr: ParaProps,
    resolved: HashMap<String, (Kind, Rc<Resolved>)>,
    default_paragraph: Option<String>,
    default_character: Option<String>,
    default_table: Option<String>,
    empty: Rc<Resolved>,
}

impl Styles {
    pub fn parse(root: Node) -> Styles {
        let mut styles = Styles::default();
        if let Some(defaults) = child(root, "docDefaults") {
            if let Some(rpr) = child(defaults, "rPrDefault").and_then(|node| child(node, "rPr")) {
                styles.default_rpr = RunProps::parse(rpr);
            }
            if let Some(ppr) = child(defaults, "pPrDefault").and_then(|node| child(node, "pPr")) {
                styles.default_ppr = ParaProps::parse(ppr);
            }
        }
        let mut definitions: HashMap<String, StyleDef> = HashMap::new();
        for style in children(root, "style") {
            let Some(id) = attr(style, "styleId") else {
                continue;
            };
            let kind = match attr(style, "type") {
                Some("character") => Kind::Character,
                Some("table") => Kind::Table,
                Some("numbering") => Kind::Numbering,
                _ => Kind::Paragraph,
            };
            if attr(style, "default").is_some_and(|value| crate::xml::on_off_value(Some(value))) {
                let slot = match kind {
                    Kind::Paragraph => &mut styles.default_paragraph,
                    Kind::Character => &mut styles.default_character,
                    Kind::Table => &mut styles.default_table,
                    Kind::Numbering => continue,
                };
                if slot.is_none() {
                    *slot = Some(id.to_owned());
                }
            }
            let parse = |name: &str| child(style, name);
            definitions.insert(
                id.to_owned(),
                StyleDef {
                    kind,
                    based_on: child(style, "basedOn").and_then(val).map(str::to_owned),
                    ppr: parse("pPr").map(ParaProps::parse).unwrap_or_default(),
                    rpr: parse("rPr").map(RunProps::parse).unwrap_or_default(),
                    tblpr: parse("tblPr").map(TableProps::parse).unwrap_or_default(),
                    tcpr: CellProps::parse(parse("tcPr")),
                },
            );
        }
        let ids: Vec<String> = definitions.keys().cloned().collect();
        for id in ids {
            let mut chain = Vec::new();
            let mut current = Some(id.clone());
            while let Some(name) = current {
                if chain.contains(&name) || chain.len() > 64 {
                    break;
                }
                let Some(definition) = definitions.get(&name) else {
                    break;
                };
                current = definition.based_on.clone();
                chain.push(name);
            }
            let mut resolved = Resolved::default();
            for name in chain.iter().rev() {
                let definition = &definitions[name];
                resolved.ppr.overlay(&definition.ppr);
                resolved.rpr.overlay(&definition.rpr);
                resolved.tblpr.overlay(&definition.tblpr);
                resolved.tcpr.margins.overlay(&definition.tcpr.margins);
                resolved.tcpr.borders.overlay(&definition.tcpr.borders);
                if definition.tcpr.shading.is_some() {
                    resolved.tcpr.shading = definition.tcpr.shading;
                }
                if definition.tcpr.v_align.is_some() {
                    resolved.tcpr.v_align = definition.tcpr.v_align;
                }
            }
            resolved.ppr.style = None;
            resolved.rpr.style = None;
            let kind = definitions[&id].kind;
            styles.resolved.insert(id, (kind, Rc::new(resolved)));
        }
        styles
    }

    fn lookup(&self, id: Option<&str>, kind: Kind, default: &Option<String>) -> Rc<Resolved> {
        let found = id
            .and_then(|id| self.resolved.get(id))
            .filter(|(found, _)| *found == kind)
            .or_else(|| default.as_deref().and_then(|id| self.resolved.get(id)));
        found.map_or_else(|| self.empty.clone(), |(_, resolved)| resolved.clone())
    }

    pub fn none(&self) -> Rc<Resolved> {
        self.empty.clone()
    }

    pub fn is_default_paragraph(&self, id: Option<&str>) -> bool {
        match id {
            None => true,
            Some(id) => {
                self.default_paragraph.as_deref() == Some(id) || !self.resolved.contains_key(id)
            }
        }
    }

    pub fn paragraph(&self, id: Option<&str>) -> Rc<Resolved> {
        self.lookup(id, Kind::Paragraph, &self.default_paragraph)
    }

    pub fn character(&self, id: Option<&str>) -> Rc<Resolved> {
        self.lookup(id, Kind::Character, &self.default_character)
    }

    pub fn table(&self, id: Option<&str>) -> Rc<Resolved> {
        self.lookup(id, Kind::Table, &self.default_table)
    }
}

#[derive(Debug, Clone)]
pub struct Level {
    pub start: i64,
    pub format: String,
    pub ppr: ParaProps,
}

#[derive(Debug, Default)]
struct Num {
    abstract_id: String,
    overrides: HashMap<u8, (Option<i64>, Option<Level>)>,
}

#[derive(Debug, Default)]
pub struct Numbering {
    abstracts: HashMap<String, (HashMap<u8, Level>, Option<String>)>,
    nums: HashMap<String, Num>,
    style_nums: HashMap<String, String>,
}

fn parse_level(node: Node) -> Option<(u8, Level)> {
    let index: u8 = attr(node, "ilvl")?.parse().ok()?;
    Some((
        index,
        Level {
            start: child(node, "start")
                .and_then(|start| number(val(start)))
                .map_or(1, |start| start as i64),
            format: child(node, "numFmt")
                .and_then(val)
                .unwrap_or("decimal")
                .to_owned(),
            ppr: child(node, "pPr").map(ParaProps::parse).unwrap_or_default(),
        },
    ))
}

impl Numbering {
    pub fn parse(root: Node, styles_root: Option<Node>) -> Numbering {
        let mut numbering = Numbering::default();
        for definition in children(root, "abstractNum") {
            let Some(id) = attr(definition, "abstractNumId") else {
                continue;
            };
            let levels = children(definition, "lvl")
                .filter_map(parse_level)
                .collect();
            let link = child(definition, "numStyleLink")
                .and_then(val)
                .map(str::to_owned);
            numbering.abstracts.insert(id.to_owned(), (levels, link));
        }
        for num in children(root, "num") {
            let Some(id) = attr(num, "numId") else {
                continue;
            };
            let Some(abstract_id) = child(num, "abstractNumId").and_then(val) else {
                continue;
            };
            let overrides = children(num, "lvlOverride")
                .filter_map(|item| {
                    let level: u8 = attr(item, "ilvl")?.parse().ok()?;
                    let start = child(item, "startOverride")
                        .and_then(|start| number(val(start)))
                        .map(|start| start as i64);
                    let replacement = child(item, "lvl")
                        .and_then(parse_level)
                        .map(|(_, level)| level);
                    Some((level, (start, replacement)))
                })
                .collect();
            numbering.nums.insert(
                id.to_owned(),
                Num {
                    abstract_id: abstract_id.to_owned(),
                    overrides,
                },
            );
        }
        if let Some(styles_root) = styles_root {
            for style in children(styles_root, "style") {
                if attr(style, "type") != Some("numbering") {
                    continue;
                }
                let (Some(id), Some(num_id)) = (
                    attr(style, "styleId"),
                    child(style, "pPr")
                        .and_then(|ppr| child(ppr, "numPr"))
                        .and_then(|numpr| child(numpr, "numId"))
                        .and_then(val),
                ) else {
                    continue;
                };
                numbering
                    .style_nums
                    .insert(id.to_owned(), num_id.to_owned());
            }
        }
        numbering
    }

    pub fn level(&self, num_id: &str, level: u8) -> Option<Level> {
        let num = self.nums.get(num_id)?;
        let mut found = self.abstract_level(&num.abstract_id, level, 0);
        if let Some((start, replacement)) = num.overrides.get(&level) {
            if let Some(replacement) = replacement {
                found = Some(replacement.clone());
            }
            if let (Some(start), Some(found)) = (start, found.as_mut()) {
                found.start = *start;
            }
        }
        found
    }

    fn abstract_level(&self, abstract_id: &str, level: u8, depth: u8) -> Option<Level> {
        let (levels, link) = self.abstracts.get(abstract_id)?;
        if let Some(found) = levels.get(&level) {
            return Some(found.clone());
        }
        let num_id = self.style_nums.get(link.as_ref()?)?;
        let num = self.nums.get(num_id)?;
        if depth > 4 {
            return None;
        }
        self.abstract_level(&num.abstract_id, level, depth + 1)
    }

    pub fn has_start_override(&self, num_id: &str, level: u8) -> bool {
        self.nums
            .get(num_id)
            .and_then(|num| num.overrides.get(&level))
            .is_some_and(|(start, _)| start.is_some())
    }

    pub fn abstract_of(&self, num_id: &str) -> Option<&str> {
        self.nums.get(num_id).map(|num| num.abstract_id.as_str())
    }
}

#[derive(Debug, Default, Clone)]
struct FontScheme {
    latin: Option<String>,
    cs: Option<String>,
    scripts: HashMap<String, String>,
}

#[derive(Debug, Default, Clone)]
pub struct Theme {
    major: FontScheme,
    minor: FontScheme,
}

fn scheme(node: Option<Node>) -> FontScheme {
    let Some(node) = node else {
        return FontScheme::default();
    };
    let typeface = |name: &str| {
        node.children()
            .find(|child| is_ns(*child, A, name))
            .and_then(|child| plain_attr(child, "typeface"))
            .filter(|face| !face.is_empty())
            .map(str::to_owned)
    };
    FontScheme {
        latin: typeface("latin"),
        cs: typeface("cs"),
        scripts: node
            .children()
            .filter(|child| is_ns(*child, A, "font"))
            .filter_map(|child| {
                Some((
                    plain_attr(child, "script")?.to_owned(),
                    plain_attr(child, "typeface")?.to_owned(),
                ))
            })
            .collect(),
    }
}

impl Theme {
    pub fn parse(root: Node) -> Theme {
        let fonts = root
            .descendants()
            .find(|node| is_ns(*node, A, "fontScheme"));
        let find = |name: &str| {
            fonts.and_then(|fonts| fonts.children().find(|child| is_ns(*child, A, name)))
        };
        Theme {
            major: scheme(find("majorFont")),
            minor: scheme(find("minorFont")),
        }
    }

    pub fn font(&self, theme: &str, lang_bidi: Option<&str>) -> Option<String> {
        let scheme = if theme.starts_with("major") {
            &self.major
        } else {
            &self.minor
        };
        if theme.ends_with("Bidi") {
            if let Some(cs) = &scheme.cs {
                return Some(cs.clone());
            }
            let script = match lang_bidi.unwrap_or("ar").split('-').next().unwrap_or("ar") {
                "he" | "yi" => "Hebr",
                "syr" => "Syrc",
                "dv" => "Thaa",
                _ => "Arab",
            };
            return scheme.scripts.get(script).cloned();
        }
        scheme.latin.clone()
    }
}
