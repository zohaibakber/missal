mod body;
mod css;
mod parts;
mod xml;

use std::io::{Cursor, Write};

use missal_core::document::{Direction, Document, PageLayout};
use missal_core::{FieldCatalog, FieldMarkers};
use zip::write::SimpleFileOptions;

use crate::export::body::Body;
use crate::export::parts::{R_NS, W_NS};

fn mm_twips(mm: f64) -> i64 {
    let value = (mm / 25.4 * 1440.0).round();
    if value.is_finite() {
        value.clamp(0.0, 31_680.0) as i64
    } else {
        0
    }
}

fn section(layout: &PageLayout, rtl: bool) -> String {
    let width = mm_twips(layout.width_mm).max(1440);
    let height = mm_twips(layout.height_mm).max(1440);
    let orient = if width > height {
        r#" w:orient="landscape""#
    } else {
        ""
    };
    format!(
        r#"<w:sectPr><w:pgSz w:w="{width}" w:h="{height}"{orient}/><w:pgMar w:top="{}" w:right="{}" w:bottom="{}" w:left="{}" w:header="720" w:footer="720" w:gutter="0"/>{}</w:sectPr>"#,
        mm_twips(layout.margin_top_mm),
        mm_twips(layout.margin_right_mm),
        mm_twips(layout.margin_bottom_mm),
        mm_twips(layout.margin_left_mm),
        if rtl { "<w:bidi/>" } else { "" }
    )
}

pub fn export(
    document: &Document,
    catalog: &FieldCatalog,
    markers: &FieldMarkers,
    title: Option<&str>,
) -> std::io::Result<Vec<u8>> {
    let layout = document.page_layout.clone().unwrap_or_else(PageLayout::a4);
    let rtl = document.root.direction != Some(Direction::Ltr);
    let content = mm_twips(layout.width_mm)
        - mm_twips(layout.margin_left_mm)
        - mm_twips(layout.margin_right_mm);
    let mut body = Body::new(catalog, markers, content);
    body.blocks(&document.blocks, rtl);
    body.finish(rtl);
    let numbering = !body.abstracts.is_empty();
    let document_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W_NS}" xmlns:r="{R_NS}"><w:body>{}{}</w:body></w:document>"#,
        body.out,
        section(&layout, rtl)
    );
    let mut parts: Vec<(&str, String)> = vec![
        ("[Content_Types].xml", parts::content_types(numbering)),
        ("_rels/.rels", parts::root_relationships()),
        ("docProps/core.xml", parts::core(title)),
        ("docProps/app.xml", parts::app()),
        ("word/document.xml", document_xml),
        (
            "word/_rels/document.xml.rels",
            parts::document_relationships(numbering, &body.links),
        ),
        ("word/styles.xml", parts::styles()),
        ("word/settings.xml", parts::settings()),
    ];
    if numbering {
        parts.push((
            "word/numbering.xml",
            parts::numbering(&body.abstracts, &body.nums),
        ));
    }
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, content) in parts {
        writer
            .start_file(name, options)
            .map_err(std::io::Error::other)?;
        writer.write_all(content.as_bytes())?;
    }
    let cursor = writer.finish().map_err(std::io::Error::other)?;
    Ok(cursor.into_inner())
}
