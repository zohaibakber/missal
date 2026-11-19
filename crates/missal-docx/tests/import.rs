use std::io::{Cursor, Write};

use missal_core::document::{Block, Direction, Inline, Paragraph, Table};
use missal_core::field::FirField;
use missal_core::{
    FieldCatalog, FieldMarkers, FieldReference, FirProperty, Placeholder, PlaceholderId,
    PlaceholderSource,
};
use missal_docx::{ImportError, Imported, Notice, import};
use zip::write::SimpleFileOptions;

const NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const CONTENT_TYPES: &str = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
const ROOT_RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

fn archive(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in parts {
        writer.start_file(*name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn document_xml(body: &str) -> String {
    format!(
        r#"<w:document xmlns:w="{NS}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body>{body}</w:body></w:document>"#
    )
}

fn docx(body: &str, extra: &[(&str, &str)]) -> Vec<u8> {
    let document = document_xml(body);
    let mut parts: Vec<(&str, &[u8])> = vec![
        ("[Content_Types].xml", CONTENT_TYPES.as_bytes()),
        ("_rels/.rels", ROOT_RELS.as_bytes()),
        ("word/document.xml", document.as_bytes()),
    ];
    for (name, content) in extra {
        parts.push((name, content.as_bytes()));
    }
    archive(&parts)
}

fn document_rels(relationships: &str) -> String {
    format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{relationships}</Relationships>"#
    )
}

fn run_import(bytes: &[u8]) -> Imported {
    import(
        bytes,
        &FieldCatalog::new(Vec::new()),
        &FieldMarkers::default(),
    )
    .unwrap()
}

fn import_body(body: &str) -> Imported {
    run_import(&docx(body, &[]))
}

fn paragraphs(blocks: &[Block]) -> Vec<&Paragraph> {
    let mut found = Vec::new();
    for block in blocks {
        match block {
            Block::Paragraph(paragraph) => found.push(paragraph),
            Block::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        found.extend(paragraphs(&cell.blocks));
                    }
                }
            }
            _ => {}
        }
    }
    found
}

fn texts(paragraph: &Paragraph) -> Vec<(&str, &str)> {
    paragraph
        .children
        .iter()
        .filter_map(|inline| match inline {
            Inline::Text(text) => Some((
                text.text.as_str(),
                text.style.as_ref().map_or("", |style| style.as_str()),
            )),
            _ => None,
        })
        .collect()
}

fn plain(paragraph: &Paragraph) -> String {
    texts(paragraph).iter().map(|(text, _)| *text).collect()
}

fn css<'s>(style: &'s str, property: &str) -> Option<&'s str> {
    style.split(';').find_map(|declaration| {
        let (name, value) = declaration.split_once(':')?;
        (name.trim() == property).then(|| value.trim())
    })
}

fn tables(blocks: &[Block]) -> Vec<&Table> {
    blocks
        .iter()
        .filter_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .collect()
}

fn run(text: &str, properties: &str) -> String {
    format!("<w:r><w:rPr>{properties}</w:rPr>{text}</w:r>")
}

#[test]
fn uses_complex_script_font_and_size_for_urdu_runs_without_changing_latin_runs() {
    let imported = import_body(
        r#"<w:p>
        <w:r><w:rPr><w:rtl/><w:rFonts w:ascii="Arial" w:cs="Jameel Noori Nastaleeq"/><w:sz w:val="22"/><w:szCs w:val="32"/><w:bCs/></w:rPr><w:t>ملزم</w:t></w:r>
        <w:r><w:rPr><w:rFonts w:ascii="Arial" w:cs="Jameel Noori Nastaleeq"/><w:sz w:val="22"/><w:szCs w:val="32"/></w:rPr><w:t>Latin</w:t></w:r>
      </w:p>"#,
    );
    let paragraph = paragraphs(&imported.document.blocks)[0];
    let runs = texts(paragraph);
    assert_eq!(runs[0].0, "ملزم");
    assert_eq!(
        css(runs[0].1, "font-family"),
        Some("\"Jameel Noori Nastaleeq\"")
    );
    assert_eq!(css(runs[0].1, "font-size"), Some("21.3333px"));
    assert_eq!(css(runs[0].1, "font-weight"), Some("700"));
    assert_eq!(runs[1].0, "Latin");
    assert_eq!(css(runs[1].1, "font-family"), Some("Arial"));
    assert_eq!(css(runs[1].1, "font-size"), Some("14.6667px"));
    assert_eq!(css(runs[1].1, "font-weight"), Some("400"));
}

#[test]
fn imports_page_geometry_rtl_table_content_and_explicit_page_breaks() {
    let imported = import_body(
        r#"<w:p><w:pPr><w:bidi/><w:spacing w:after="120"/></w:pPr><w:r><w:rPr><w:rtl/><w:szCs w:val="32"/></w:rPr><w:t>نام {{ملزم}}</w:t></w:r></w:p>
    <w:tbl><w:tblPr><w:tblW w:w="4000" w:type="dxa"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>گواہ</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
    <w:p><w:r><w:br w:type="page"/></w:r></w:p>
    <w:p><w:r><w:t>اگلا صفحہ</w:t></w:r></w:p>
    <w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="720" w:right="900" w:bottom="720" w:left="900"/></w:sectPr>"#,
    );
    let document = &imported.document;
    let layout = document.page_layout.as_ref().unwrap();
    assert!((layout.width_mm - 210.0).abs() < 0.5);
    assert!((layout.margin_top_mm - 12.7).abs() < 0.01);
    assert!((layout.margin_right_mm - 15.875).abs() < 0.01);
    assert_eq!(tables(&document.blocks).len(), 1);
    let breaks = document
        .blocks
        .iter()
        .filter(|block| matches!(block, Block::PageBreak(_)))
        .count();
    assert_eq!(breaks, 1);
    let all = paragraphs(&document.blocks);
    assert_eq!(all[0].props.direction, Some(Direction::Rtl));
    assert_eq!(css(texts(all[0])[0].1, "font-size"), Some("21.3333px"));
    let text: String = all.iter().map(|paragraph| plain(paragraph)).collect();
    assert!(text.contains("{{ملزم}}"));
    assert!(text.contains("اگلا صفحہ"));
    assert!(imported.notices.is_empty());
}

#[test]
fn moves_a_known_field_name_split_across_differently_formatted_runs_into_its_first_run() {
    let imported = import_body(
        r#"<w:p>
        <w:r><w:rPr><w:rtl/></w:rPr><w:t xml:space="preserve">مورخہ: «</w:t></w:r>
        <w:r><w:rPr><w:rtl w:val="0"/></w:rPr><w:t>Date</w:t></w:r>
        <w:r><w:rPr><w:rtl w:val="0"/></w:rPr><w:t>_</w:t></w:r>
        <w:r><w:rPr><w:rtl w:val="0"/></w:rPr><w:t>FIR</w:t></w:r>
        <w:r><w:rPr><w:rtl/></w:rPr><w:t xml:space="preserve">» اور «نامعلوم</w:t></w:r>
        <w:r><w:t>»</w:t></w:r>
      </w:p>"#,
    );
    let paragraph = paragraphs(&imported.document.blocks)[0];
    let shape: Vec<String> = paragraph
        .children
        .iter()
        .map(|inline| match inline {
            Inline::Text(text) => text.text.clone(),
            Inline::Field(field) => format!("{:?}", field.reference),
            other => format!("{other:?}"),
        })
        .collect();
    let date = FieldReference::Fir(FirField {
        property: FirProperty::Date,
        index: None,
    });
    assert_eq!(
        shape,
        [
            "مورخہ: ".to_owned(),
            format!("{date:?}"),
            " اور «نامعلوم".to_owned(),
            "»".to_owned()
        ]
    );
    let Inline::Field(field) = &paragraph.children[1] else {
        panic!()
    };
    let Inline::Text(first) = &paragraph.children[0] else {
        panic!()
    };
    assert_eq!(field.imported_style, first.style);
}

#[test]
fn replaces_word_merge_fields_and_their_sample_values_with_field_tokens() {
    let begin = run(r#"<w:fldChar w:fldCharType="begin"/>"#, "");
    let separate = run(r#"<w:fldChar w:fldCharType="separate"/>"#, "");
    let end = run(r#"<w:fldChar w:fldCharType="end"/>"#, "");
    let body = format!(
        r#"<w:p>{begin}{}{}{}{separate}{}{end}{}<w:fldSimple w:instr=' MERGEFIELD "Date FIR" '>{}</w:fldSimple>{begin}{}{separate}{}{end}</w:p>"#,
        run(r#"<w:instrText xml:space="preserve"> </w:instrText>"#, ""),
        run("<w:instrText>MERGEFIELD</w:instrText>", ""),
        run(
            r#"<w:instrText xml:space="preserve"> تھانہ_نام_ \* MERGEFORMAT</w:instrText>"#,
            ""
        ),
        run("<w:t>تھانہ گلشن اقبال</w:t>", "<w:b/><w:bCs/>"),
        run(r#"<w:t xml:space="preserve"> اور </w:t>"#, ""),
        run("<w:t>26.07.2026</w:t>", ""),
        run("<w:instrText>PAGE</w:instrText>", ""),
        run("<w:t>3</w:t>", ""),
    );
    let catalog = FieldCatalog::new(vec![Placeholder {
        id: PlaceholderId::new(7).unwrap(),
        label: "تھانہ نام".to_owned(),
        source: PlaceholderSource::Custom,
    }]);
    let imported = import(&docx(&body, &[]), &catalog, &FieldMarkers::default()).unwrap();
    let paragraph = paragraphs(&imported.document.blocks)[0];
    let shape: Vec<String> = paragraph
        .children
        .iter()
        .map(|inline| match inline {
            Inline::Text(text) => text.text.clone(),
            Inline::Field(field) => format!("{:?}", field.reference),
            other => format!("{other:?}"),
        })
        .collect();
    let station = FieldReference::Catalog(PlaceholderId::new(7).unwrap());
    let date = FieldReference::Fir(FirField {
        property: FirProperty::Date,
        index: None,
    });
    assert_eq!(
        shape,
        [
            format!("{station:?}"),
            " اور ".to_owned(),
            format!("{date:?}"),
            "3".to_owned()
        ]
    );
    let Inline::Field(token) = &paragraph.children[0] else {
        panic!()
    };
    assert_eq!(
        css(
            token.imported_style.as_ref().unwrap().as_str(),
            "font-weight"
        ),
        Some("700")
    );
}

#[test]
fn unknown_merge_fields_become_unresolved_tokens_and_cross_paragraph_fields_keep_their_text() {
    let imported = import_body(
        r#"<w:p><w:fldSimple w:instr="MERGEFIELD  نامعلوم__خانہ"><w:r><w:t>x</w:t></w:r></w:fldSimple></w:p>
        <w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText>MERGEFIELD a</w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>first</w:t></w:r></w:p>
        <w:p><w:r><w:t>second</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>"#,
    );
    let all = paragraphs(&imported.document.blocks);
    let Inline::Field(field) = &all[0].children[0] else {
        panic!()
    };
    assert_eq!(
        field.reference,
        FieldReference::Unresolved("نامعلوم خانہ".to_owned())
    );
    assert_eq!(plain(all[1]), "first");
    assert_eq!(plain(all[2]), "second");
}

#[test]
fn salvages_text_from_odd_documents_and_rejects_unreadable_files() {
    assert_eq!(
        import(
            b"not a zip",
            &FieldCatalog::new(Vec::new()),
            &FieldMarkers::default()
        )
        .unwrap_err(),
        ImportError::NotDocx
    );
    let empty = archive(&[("other.txt", b"x")]);
    assert_eq!(
        import(
            &empty,
            &FieldCatalog::new(Vec::new()),
            &FieldMarkers::default()
        )
        .unwrap_err(),
        ImportError::MissingDocument
    );
    let broken = archive(&[("word/document.xml", b"<w:document")]);
    assert!(matches!(
        import(
            &broken,
            &FieldCatalog::new(Vec::new()),
            &FieldMarkers::default()
        ),
        Err(ImportError::InvalidDocument(_))
    ));
    let styles = "<w:styles";
    let rels = document_rels(
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#,
    );
    let body = r#"<w:p><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>kept</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:tcPr><w:vMerge/></w:tcPr></w:tc></w:tr></w:tbl>"#;
    let imported = run_import(&docx(
        body,
        &[
            ("word/styles.xml", styles),
            ("word/_rels/document.xml.rels", &rels),
        ],
    ));
    assert_eq!(plain(paragraphs(&imported.document.blocks)[0]), "kept");
    assert_eq!(imported.notices, [Notice::StylesUnreadable]);
    let blank = import_body("");
    assert_eq!(blank.document.blocks.len(), 1);
}
