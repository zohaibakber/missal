use std::fmt::Write;

use super::xml::escape;

pub const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
pub const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const RELS_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const MAIN_TYPE: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml";

pub const DEFAULT_FONT: &str = "Jameel Noori Nastaleeq";
pub const DEFAULT_HALF_POINTS: u32 = 30;
pub const DEFAULT_LINE: u32 = 340;
pub const DEFAULT_AFTER: u32 = 120;
pub const HEADING_SCALES: [f64; 6] = [2.0, 1.5, 1.25, 1.0, 0.83, 0.67];

pub fn content_types(numbering: bool) -> String {
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>"#,
    );
    let mut part = |name: &str, kind: &str| {
        let _ = write!(out, r#"<Override PartName="{name}" ContentType="{kind}"/>"#);
    };
    part(
        "/word/document.xml",
        &format!("{MAIN_TYPE}.document.main+xml"),
    );
    part("/word/styles.xml", &format!("{MAIN_TYPE}.styles+xml"));
    part("/word/settings.xml", &format!("{MAIN_TYPE}.settings+xml"));
    if numbering {
        part("/word/numbering.xml", &format!("{MAIN_TYPE}.numbering+xml"));
    }
    part(
        "/docProps/core.xml",
        "application/vnd.openxmlformats-package.core-properties+xml",
    );
    part(
        "/docProps/app.xml",
        "application/vnd.openxmlformats-officedocument.extended-properties+xml",
    );
    out.push_str("</Types>");
    out
}

pub fn root_relationships() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="{REL}/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="{REL}/extended-properties" Target="docProps/app.xml"/></Relationships>"#
    )
}

pub fn document_relationships(numbering: bool, links: &[String]) -> String {
    let mut out = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="{REL}/styles" Target="styles.xml"/><Relationship Id="rId2" Type="{REL}/settings" Target="settings.xml"/>"#
    );
    if numbering {
        let _ = write!(
            out,
            r#"<Relationship Id="rId3" Type="{REL}/numbering" Target="numbering.xml"/>"#
        );
    }
    for (index, target) in links.iter().enumerate() {
        let _ = write!(
            out,
            r#"<Relationship Id="{}" Type="{REL}/hyperlink" Target="{}" TargetMode="External"/>"#,
            link_id(index),
            escape(target)
        );
    }
    out.push_str("</Relationships>");
    out
}

pub fn link_id(index: usize) -> String {
    format!("rIdLink{}", index + 1)
}

pub fn core(title: Option<&str>) -> String {
    let title = title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(|title| format!("<dc:title>{}</dc:title>", escape(title)))
        .unwrap_or_default();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">{title}</cp:coreProperties>"#
    )
}

pub fn app() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>Missal</Application></Properties>"#.to_owned()
}

pub fn settings() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings xmlns:w="{W_NS}"><w:defaultTabStop w:val="720"/><w:characterSpacingControl w:val="doNotCompress"/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>"#
    )
}

pub fn styles() -> String {
    let font = DEFAULT_FONT;
    let size = DEFAULT_HALF_POINTS;
    let mut out = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="{W_NS}"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="{font}" w:hAnsi="{font}" w:eastAsia="{font}" w:cs="{font}"/><w:sz w:val="{size}"/><w:szCs w:val="{size}"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:bidi/><w:spacing w:before="0" w:after="{DEFAULT_AFTER}" w:line="{DEFAULT_LINE}" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="character" w:default="1" w:styleId="DefaultParagraphFont"><w:name w:val="Default Paragraph Font"/><w:uiPriority w:val="1"/><w:semiHidden/></w:style><w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:semiHidden/><w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>"#
    );
    for (index, scale) in HEADING_SCALES.iter().enumerate() {
        let level = index + 1;
        let half_points = (f64::from(size) * scale).round();
        let _ = write!(
            out,
            r#"<w:style w:type="paragraph" w:styleId="Heading{level}"><w:name w:val="heading {level}"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="9"/><w:qFormat/><w:pPr><w:keepNext/><w:keepLines/><w:spacing w:after="0"/><w:outlineLvl w:val="{index}"/></w:pPr><w:rPr><w:b/><w:bCs/><w:sz w:val="{half_points}"/><w:szCs w:val="{half_points}"/></w:rPr></w:style>"#
        );
    }
    out.push_str(r#"<w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="29"/><w:qFormat/><w:pPr><w:pBdr><w:left w:val="single" w:sz="18" w:space="8" w:color="000000"/></w:pBdr><w:spacing w:after="0"/><w:ind w:left="240"/></w:pPr></w:style></w:styles>"#);
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelKind {
    Bullet,
    Number,
    Check,
}

#[derive(Debug, Clone, Copy)]
pub struct Level {
    pub kind: LevelKind,
    pub start: i64,
}

pub struct Abstract {
    pub levels: [Option<Level>; 9],
}

pub struct Num {
    pub abstract_index: usize,
    pub checked: bool,
}

const BULLETS: [&str; 3] = ["\u{2022}", "\u{25E6}", "\u{25AA}"];

fn level_text(kind: LevelKind, level: usize, checked: bool) -> (&'static str, String) {
    match kind {
        LevelKind::Bullet => ("bullet", BULLETS[level % BULLETS.len()].to_owned()),
        LevelKind::Check => (
            "bullet",
            if checked { "\u{2611}" } else { "\u{2610}" }.to_owned(),
        ),
        LevelKind::Number => ("decimal", format!("%{}.", level + 1)),
    }
}

fn level_xml(out: &mut String, index: usize, level: Level, checked: bool) {
    let (format, text) = level_text(level.kind, index, checked);
    let start = level.start;
    let left = 720 * (index + 1);
    let _ = write!(
        out,
        r#"<w:lvl w:ilvl="{index}"><w:start w:val="{start}"/><w:numFmt w:val="{format}"/><w:lvlText w:val="{}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{left}" w:hanging="360"/></w:pPr></w:lvl>"#,
        escape(&text)
    );
}

pub fn numbering(abstracts: &[Abstract], nums: &[Num]) -> String {
    let mut out = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering xmlns:w="{W_NS}">"#
    );
    let fallback = |levels: &[Option<Level>; 9], index: usize| {
        levels[..=index]
            .iter()
            .rev()
            .find_map(|level| *level)
            .unwrap_or(Level {
                kind: LevelKind::Bullet,
                start: 1,
            })
    };
    for (id, definition) in abstracts.iter().enumerate() {
        let _ = write!(
            out,
            r#"<w:abstractNum w:abstractNumId="{id}"><w:multiLevelType w:val="hybridMultilevel"/>"#
        );
        for index in 0..9 {
            let level = definition.levels[index].unwrap_or_else(|| {
                let inherited = fallback(&definition.levels, index);
                Level {
                    kind: inherited.kind,
                    start: 1,
                }
            });
            level_xml(&mut out, index, level, false);
        }
        out.push_str("</w:abstractNum>");
    }
    for (index, num) in nums.iter().enumerate() {
        let _ = write!(
            out,
            r#"<w:num w:numId="{}"><w:abstractNumId w:val="{}"/>"#,
            index + 1,
            num.abstract_index
        );
        if num.checked {
            let levels = &abstracts[num.abstract_index].levels;
            for (level_index, level) in levels.iter().enumerate() {
                if let Some(level) = level.filter(|level| level.kind == LevelKind::Check) {
                    let _ = write!(out, r#"<w:lvlOverride w:ilvl="{level_index}">"#);
                    level_xml(&mut out, level_index, level, true);
                    out.push_str("</w:lvlOverride>");
                }
            }
        }
        out.push_str("</w:num>");
    }
    out.push_str("</w:numbering>");
    out
}
