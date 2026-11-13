mod convert;
mod format;
mod media;
mod package;
mod props;
mod styles;
mod xml;

use std::fmt;

use missal_core::document::{
    Block, Direction, Document, ElementProps, Inline, PageLayout, Paragraph,
};
use missal_core::{FieldCatalog, FieldMarkers};
use roxmltree::{Document as XmlDocument, ParsingOptions};

use crate::convert::{Converter, Section, section};
use crate::package::Package;
use crate::styles::{Numbering, Styles, Theme};
use crate::xml::{child, is_w, w_name};

pub use crate::convert::merge_field_name;
pub use crate::format::URDU_FONT_WINDOWS_LINE_RATIO;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    NotDocx,
    MissingDocument,
    InvalidDocument(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::NotDocx => {
                f.write_str("This file is not a readable Word (.docx) document.")
            }
            ImportError::MissingDocument => f.write_str("This Word file has no document body."),
            ImportError::InvalidDocument(reason) => {
                write!(f, "This Word file's document XML is unreadable: {reason}")
            }
        }
    }
}

impl std::error::Error for ImportError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Notice {
    DifferentPageLayouts,
    HeadersFootersAsContent,
    DrawingsNeedChecking,
    ImageOmitted,
    FloatingFlattened,
    FootnotesOmitted,
    EmbeddedContentOmitted,
    StylesUnreadable,
}

impl Notice {
    pub fn message(self) -> &'static str {
        match self {
            Notice::DifferentPageLayouts => {
                "This document uses different page sizes or margins. Printing uses the first page's settings."
            }
            Notice::HeadersFootersAsContent => {
                "Headers and footers were added as editable content at the start and end of the document."
            }
            Notice::DrawingsNeedChecking => {
                "Some drawings or equations may need to be checked after import."
            }
            Notice::ImageOmitted => "An image could not be embedded and was omitted.",
            Notice::FloatingFlattened => "Floating layout was flattened to keep text.",
            Notice::FootnotesOmitted => "Footnotes and endnotes were not imported.",
            Notice::EmbeddedContentOmitted => "Embedded objects or documents were omitted.",
            Notice::StylesUnreadable => {
                "Some formatting could not be read, so default formatting was used."
            }
        }
    }
}

impl fmt::Display for Notice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

#[derive(Debug, Clone)]
pub struct Imported {
    pub document: Document,
    pub notices: Vec<Notice>,
    pub suggested_name: Option<String>,
}

fn parse(text: &str) -> Result<XmlDocument<'_>, roxmltree::Error> {
    XmlDocument::parse_with_options(
        text,
        ParsingOptions {
            allow_dtd: false,
            ..ParsingOptions::default()
        },
    )
}

pub fn import(
    bytes: &[u8],
    catalog: &FieldCatalog,
    markers: &FieldMarkers,
) -> Result<Imported, ImportError> {
    let mut package = Package::open(bytes)?;
    let main = package
        .main_document()
        .ok_or(ImportError::MissingDocument)?;
    let text = package
        .read_text(&main)
        .ok_or(ImportError::MissingDocument)?;
    let xml = parse(&text).map_err(|error| ImportError::InvalidDocument(error.to_string()))?;
    let root = xml.root_element();
    let body = child(root, "body").ok_or(ImportError::MissingDocument)?;
    let rels = package.relationships(&main);
    let mut notices = Vec::new();

    let mut load = |kind: &str| -> Option<String> {
        let target = rels.of_kind(kind)?.target.clone();
        package.read_text(&target)
    };
    let styles_text = load("/styles");
    let numbering_text = load("/numbering");
    let theme_text = load("/theme");
    let core_target = package
        .relationships("")
        .of_kind("/core-properties")
        .map(|relationship| relationship.target.clone())
        .unwrap_or_else(|| "docProps/core.xml".to_owned());
    let core_text = package.read_text(&core_target);

    let styles_xml = styles_text.as_deref().map(parse);
    let numbering_xml = numbering_text.as_deref().map(parse);
    let theme_xml = theme_text.as_deref().map(parse);
    if matches!(styles_xml, Some(Err(_))) || matches!(numbering_xml, Some(Err(_))) {
        notices.push(crate::Notice::StylesUnreadable);
    }
    let styles_root = styles_xml
        .as_ref()
        .and_then(|xml| xml.as_ref().ok())
        .map(XmlDocument::root_element);
    let styles = styles_root.map(Styles::parse).unwrap_or_default();
    let numbering = numbering_xml
        .as_ref()
        .and_then(|xml| xml.as_ref().ok())
        .map(|xml| Numbering::parse(xml.root_element(), styles_root))
        .unwrap_or_default();
    let theme = theme_xml
        .as_ref()
        .and_then(|xml| xml.as_ref().ok())
        .map(|xml| Theme::parse(xml.root_element()))
        .unwrap_or_default();

    let sections: Vec<Section> = body
        .descendants()
        .filter(|node| {
            is_w(*node, "sectPr")
                && node
                    .parent()
                    .is_some_and(|parent| matches!(w_name(parent), Some("pPr" | "body")))
        })
        .map(section)
        .collect();
    let layout = sections
        .first()
        .map_or_else(PageLayout::a4, |section| section.layout.clone());
    if sections.iter().any(|section| section.layout != layout) {
        notices.push(Notice::DifferentPageLayouts);
    }
    let header = sections.first().and_then(|section| section.header.clone());
    let footer = sections.first().and_then(|section| section.footer.clone());

    let mut converter = Converter::new(
        &mut package,
        styles,
        numbering,
        theme,
        catalog,
        markers,
        rels.clone(),
        sections,
    );
    converter.notices = notices;
    let body_blocks = converter.body(body);
    let header_blocks = header
        .map(|id| decorations(&mut converter, &rels, &id))
        .unwrap_or_default();
    let footer_blocks = footer
        .map(|id| decorations(&mut converter, &rels, &id))
        .unwrap_or_default();
    if has_content(&header_blocks) || has_content(&footer_blocks) {
        converter.notice(Notice::HeadersFootersAsContent);
    }
    let mut blocks = Vec::new();
    if has_content(&header_blocks) {
        blocks.extend(header_blocks);
    }
    blocks.extend(body_blocks);
    if has_content(&footer_blocks) {
        blocks.extend(footer_blocks);
    }
    let notices = std::mem::take(&mut converter.notices);
    drop(converter);

    let mut blocks = normalize_page_breaks(blocks);
    if blocks.is_empty() {
        blocks.push(Block::Paragraph(Paragraph {
            props: ElementProps {
                direction: Some(Direction::Rtl),
                ..ElementProps::default()
            },
            children: Vec::new(),
        }));
    }
    let mut document = Document::new(blocks);
    document.page_layout = Some(layout);
    Ok(Imported {
        document,
        notices,
        suggested_name: core_text.as_deref().and_then(title),
    })
}

fn decorations(converter: &mut Converter, rels: &package::Relationships, id: &str) -> Vec<Block> {
    let Some(target) = rels.get(id).filter(|relationship| !relationship.external) else {
        return Vec::new();
    };
    let target = target.target.clone();
    let Some(text) = converter.read_text(&target) else {
        return Vec::new();
    };
    let Ok(xml) = parse(&text) else {
        return Vec::new();
    };
    let part_rels = converter.relationships(&target);
    let saved = std::mem::replace(&mut converter.rels, part_rels);
    let blocks = converter.part(xml.root_element());
    converter.rels = saved;
    blocks
}

fn title(core: &str) -> Option<String> {
    let xml = parse(core).ok()?;
    let title = xml
        .root_element()
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "title")?;
    let text = title
        .text()?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!text.is_empty()).then_some(text)
}

fn inline_content(inline: &Inline) -> bool {
    match inline {
        Inline::Text(text) => text
            .text
            .chars()
            .any(|c| !c.is_whitespace() && c != '\u{2003}'),
        Inline::Field(_) | Inline::Image(_) => true,
        Inline::Link(link) => link.children.iter().any(inline_content),
        Inline::LineBreak(_) | Inline::Other(_) => false,
    }
}

fn block_content(block: &Block) -> bool {
    match block {
        Block::Paragraph(paragraph) => paragraph.children.iter().any(inline_content),
        Block::Heading(heading) => heading.children.iter().any(inline_content),
        Block::Quote(quote) => quote.children.iter().any(inline_content),
        Block::PageBreak(_) => false,
        _ => true,
    }
}

fn has_content(blocks: &[Block]) -> bool {
    blocks.iter().any(block_content)
}

fn normalize_page_breaks(blocks: Vec<Block>) -> Vec<Block> {
    let mut result: Vec<Block> = Vec::with_capacity(blocks.len());
    let mut seen = false;
    for block in blocks {
        if matches!(block, Block::PageBreak(_)) {
            if !seen || matches!(result.last(), Some(Block::PageBreak(_))) {
                continue;
            }
        } else if block_content(&block) {
            seen = true;
        }
        result.push(block);
    }
    let mut following = false;
    let mut kept: Vec<Block> = Vec::with_capacity(result.len());
    for block in result.into_iter().rev() {
        if matches!(block, Block::PageBreak(_)) {
            if !following {
                continue;
            }
        } else if block_content(&block) {
            following = true;
        }
        kept.push(block);
    }
    kept.reverse();
    kept
}
