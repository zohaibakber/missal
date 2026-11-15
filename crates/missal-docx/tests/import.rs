use std::io::{Cursor, Write};

use missal_core::document::{
    Align, Block, Direction, Inline, ListItemChild, ListType, Paragraph, Table,
};
use missal_core::field::FirField;
use missal_core::{
    FieldCatalog, FieldMarkers, FieldReference, FirProperty, Placeholder, PlaceholderId,
    PlaceholderSource,
};
use missal_docx::{ImportError, Imported, Notice, URDU_FONT_WINDOWS_LINE_RATIO, import};
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

fn styles(content: &str) -> String {
    format!(r#"<w:styles xmlns:w="{NS}">{content}</w:styles>"#)
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
fn honors_explicit_disabled_rtl_and_complex_script_bold_flags() {
    let imported = import_body(
        r#"<w:p>
        <w:r><w:rPr><w:rtl w:val="0"/><w:sz w:val="22"/><w:szCs w:val="32"/></w:rPr><w:t>abc</w:t></w:r>
        <w:r><w:rPr><w:rtl/><w:b/><w:bCs w:val="0"/></w:rPr><w:t>گواہ</w:t></w:r>
      </w:p>"#,
    );
    let runs = texts(paragraphs(&imported.document.blocks)[0]);
    assert_eq!(css(runs[0].1, "font-size"), Some("14.6667px"));
    assert_eq!(css(runs[1].1, "font-weight"), Some("400"));
}

#[test]
fn arabic_script_text_always_uses_complex_script_formatting() {
    let imported = import_body(
        r#"<w:p><w:r><w:rPr><w:rtl w:val="0"/><w:sz w:val="22"/><w:szCs w:val="32"/></w:rPr><w:t>نام Name</w:t></w:r></w:p>"#,
    );
    let runs = texts(paragraphs(&imported.document.blocks)[0]);
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].0, "نام ");
    assert_eq!(css(runs[0].1, "font-size"), Some("21.3333px"));
    assert_eq!(runs[1].0, "Name");
    assert_eq!(css(runs[1].1, "font-size"), Some("14.6667px"));
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
fn keeps_text_inside_right_to_left_embeddings_and_lays_out_right_to_left_tables() {
    let imported = import_body(&format!(
        r#"<w:p><w:dir w:val="rtl">{}</w:dir><w:bdo w:val="rtl">{}</w:bdo></w:p>
        <w:tbl><w:tblPr><w:bidiVisual/><w:tblInd w:w="-683" w:type="dxa"/></w:tblPr><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl>"#,
        run("<w:t>گواہ</w:t>", ""),
        run("<w:t>ان</w:t>", "")
    ));
    let blocks = &imported.document.blocks;
    assert_eq!(plain(paragraphs(blocks)[0]), "گواہان");
    let table = tables(blocks)[0];
    assert_eq!(table.props.direction, Some(Direction::Rtl));
    let style = table.props.imported_style.as_ref().unwrap().as_str();
    assert_eq!(css(style, "margin"), Some("0px -45.5333px 0px 0px"));
}

#[test]
fn sizes_words_auto_line_spacing_from_the_fonts_own_line_height() {
    let font = r#"<w:rFonts w:cs="Jameel Noori Nastaleeq"/><w:szCs w:val="30"/><w:rtl/>"#;
    let imported = import_body(&format!(
        r#"<w:p><w:pPr><w:spacing w:line="259" w:lineRule="auto"/></w:pPr>{}</w:p>
        <w:p>{}</w:p>
        <w:p><w:pPr><w:spacing w:line="360" w:lineRule="exact"/></w:pPr>{}</w:p>"#,
        run("<w:t>ا</w:t>", font),
        run("<w:t>ب</w:t>", font),
        run("<w:t>ج</w:t>", font)
    ));
    let all = paragraphs(&imported.document.blocks);
    let height = |index: usize| {
        css(texts(all[index])[0].1, "line-height")
            .unwrap()
            .to_owned()
    };
    let round = |value: f64| format!("{}px", (value * 100.0).round() / 100.0);
    assert_eq!(
        height(0),
        round(259.0 / 240.0 * 20.0 * URDU_FONT_WINDOWS_LINE_RATIO)
    );
    assert_eq!(height(1), round(20.0 * URDU_FONT_WINDOWS_LINE_RATIO));
    assert_eq!(height(2), "24px");
}

#[test]
fn resolves_style_chains_defaults_and_toggle_properties() {
    let styles = styles(
        r#"<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:cs="Arial"/><w:sz w:val="22"/><w:szCs w:val="22"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="160"/></w:pPr></w:pPrDefault></w:docDefaults>
        <w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:pPr><w:bidi/></w:pPr><w:rPr><w:szCs w:val="28"/></w:rPr></w:style>
        <w:style w:type="paragraph" w:styleId="Title"><w:basedOn w:val="Normal"/><w:pPr><w:jc w:val="center"/></w:pPr><w:rPr><w:b/><w:bCs/><w:i/></w:rPr></w:style>
        <w:style w:type="paragraph" w:styleId="Subtitle"><w:basedOn w:val="Title"/><w:rPr><w:i w:val="0"/></w:rPr></w:style>
        <w:style w:type="character" w:styleId="Strong"><w:rPr><w:b/><w:bCs/></w:rPr></w:style>"#,
    );
    let body = r#"<w:p><w:r><w:rPr><w:rtl/></w:rPr><w:t>عام</w:t></w:r></w:p>
        <w:p><w:pPr><w:pStyle w:val="Title"/></w:pPr><w:r><w:t>Title</w:t></w:r><w:r><w:rPr><w:rStyle w:val="Strong"/></w:rPr><w:t>Plain</w:t></w:r><w:r><w:rPr><w:b w:val="0"/></w:rPr><w:t>Direct</w:t></w:r></w:p>
        <w:p><w:pPr><w:pStyle w:val="Subtitle"/></w:pPr><w:r><w:t>Sub</w:t></w:r></w:p>"#;
    let rels = document_rels(
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#,
    );
    let imported = run_import(&docx(
        body,
        &[
            ("word/styles.xml", &styles),
            ("word/_rels/document.xml.rels", &rels),
        ],
    ));
    let all = paragraphs(&imported.document.blocks);
    assert_eq!(all[0].props.direction, Some(Direction::Rtl));
    let normal = texts(all[0]);
    assert_eq!(css(normal[0].1, "font-family"), Some("Arial"));
    assert_eq!(css(normal[0].1, "font-size"), Some("18.6667px"));
    let paragraph_style = all[0].props.imported_style.as_ref().unwrap().as_str();
    assert_eq!(css(paragraph_style, "margin"), Some("0px 0px 10.6667px"));
    assert_eq!(all[1].props.align, Some(Align::Center));
    let title = texts(all[1]);
    assert_eq!(title[0].0, "Title");
    assert_eq!(css(title[0].1, "font-weight"), Some("700"));
    assert_eq!(css(title[0].1, "font-style"), Some("italic"));
    assert_eq!(title[1].0, "PlainDirect");
    assert_eq!(css(title[1].1, "font-weight"), Some("400"));
    let sub = texts(all[2]);
    assert_eq!(css(sub[0].1, "font-weight"), Some("700"));
    assert_eq!(css(sub[0].1, "font-style"), None);
}

#[test]
fn right_aligned_bidi_paragraphs_align_to_the_end_side() {
    let imported = import_body(
        r#"<w:p><w:pPr><w:bidi/><w:jc w:val="right"/></w:pPr><w:r><w:t>دستخط</w:t></w:r></w:p>
        <w:p><w:pPr><w:bidi/><w:jc w:val="left"/><w:ind w:left="720" w:right="360" w:hanging="360"/></w:pPr><w:r><w:t>آغاز</w:t></w:r></w:p>
        <w:p><w:pPr><w:jc w:val="right"/><w:ind w:left="720"/></w:pPr><w:r><w:t>Right</w:t></w:r></w:p>"#,
    );
    let all = paragraphs(&imported.document.blocks);
    assert_eq!(all[0].props.align, Some(Align::End));
    assert_eq!(all[1].props.align, Some(Align::Start));
    let style = all[1].props.imported_style.as_ref().unwrap().as_str();
    assert_eq!(css(style, "margin"), Some("0px 48px 0px 24px"));
    assert_eq!(css(style, "text-indent"), Some("-24px"));
    assert_eq!(all[2].props.direction, Some(Direction::Ltr));
    assert_eq!(all[2].props.align, Some(Align::End));
    let style = all[2].props.imported_style.as_ref().unwrap().as_str();
    assert_eq!(css(style, "margin"), Some("0px 0px 0px 48px"));
}

#[test]
fn numbered_paragraphs_become_lists_that_continue_across_interruptions() {
    let numbering = format!(
        r#"<w:numbering xmlns:w="{NS}"><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="bullet"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#
    );
    let item = |level: u8, text: &str| {
        format!(
            r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"#
        )
    };
    let body = [
        item(0, "one"),
        item(1, "inner"),
        item(0, "two"),
        "<w:p><w:r><w:t>gap</w:t></w:r></w:p>".to_owned(),
        item(0, "three"),
    ]
    .concat();
    let rels = document_rels(
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>"#,
    );
    let imported = run_import(&docx(
        &body,
        &[
            ("word/numbering.xml", &numbering),
            ("word/_rels/document.xml.rels", &rels),
        ],
    ));
    let blocks = &imported.document.blocks;
    assert_eq!(blocks.len(), 3);
    let Block::List(first) = &blocks[0] else {
        panic!("{:?}", blocks[0])
    };
    assert_eq!(first.list_type, Some(ListType::Number));
    assert_eq!(first.start, Some(1));
    let items: Vec<_> = first.items.iter().collect();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].value, Some(1));
    let ListItemChild::List(nested) = &items[1].children[0] else {
        panic!()
    };
    assert_eq!(nested.list_type, Some(ListType::Bullet));
    assert_eq!(items[2].value, Some(2));
    let style = items[0].props.imported_style.as_ref().unwrap().as_str();
    assert_eq!(css(style, "text-indent"), Some("-24px"));
    let Block::List(second) = &blocks[2] else {
        panic!()
    };
    assert_eq!(second.start, Some(3));
}

#[test]
fn tables_keep_grid_widths_spans_merges_borders_and_shading() {
    let border = r#"<w:top w:val="single" w:sz="12" w:color="FF0000"/>"#;
    let imported = import_body(&format!(
        r#"<w:tbl><w:tblPr><w:tblBorders><w:insideH w:val="single" w:sz="4"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="1500"/><w:gridCol w:w="3000"/><w:gridCol w:w="1500"/></w:tblGrid>
        <w:tr><w:trPr><w:gridAfter w:val="1"/></w:trPr><w:tc><w:tcPr><w:gridSpan w:val="2"/><w:shd w:val="clear" w:fill="FFFF00"/><w:tcBorders>{border}</w:tcBorders></w:tcPr><w:p><w:r><w:t>wide</w:t></w:r></w:p></w:tc></w:tr>
        <w:tr><w:tc><w:tcPr><w:vMerge w:val="restart"/><w:vAlign w:val="center"/></w:tcPr><w:p><w:r><w:t>tall</w:t></w:r></w:p></w:tc><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr>
        <w:tr><w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p/></w:tc><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr></w:tbl>"#
    ));
    let table = tables(&imported.document.blocks)[0];
    assert_eq!(table.col_widths, Some(vec![100.0, 200.0, 100.0]));
    let rows: Vec<_> = table.rows.iter().collect();
    let cells = |row: usize| rows[row].cells.iter().collect::<Vec<_>>();
    assert_eq!(cells(0).len(), 2);
    assert_eq!(cells(0)[0].col_span, Some(2));
    assert_eq!(
        cells(0)[0].background_color.as_deref(),
        Some("rgb(255, 255, 0)")
    );
    let style = cells(0)[0].props.imported_style.as_ref().unwrap().as_str();
    assert_eq!(css(style, "border-width"), Some("2px 0px 0.67px"));
    assert_eq!(
        css(style, "border-color"),
        Some("rgb(255, 0, 0) rgb(0, 0, 0) rgb(0, 0, 0)")
    );
    assert_eq!(cells(1)[0].row_span, Some(2));
    assert_eq!(cells(1)[0].vertical_align.as_deref(), Some("middle"));
    assert_eq!(cells(2).len(), 2);
}

#[test]
fn page_breaks_from_breaks_paragraph_properties_and_sections_are_deduplicated_and_trimmed() {
    let imported = import_body(
        r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>
        <w:p><w:r><w:t>one</w:t></w:r><w:r><w:br w:type="page"/></w:r><w:r><w:t>two</w:t></w:r></w:p>
        <w:p><w:pPr><w:pageBreakBefore/></w:pPr><w:r><w:t>three</w:t></w:r></w:p>
        <w:p><w:pPr><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr></w:pPr><w:r><w:t>four</w:t></w:r></w:p>
        <w:p><w:pPr><w:pageBreakBefore/></w:pPr><w:r><w:t>five</w:t></w:r></w:p>
        <w:p><w:pPr><w:sectPr><w:pgSz w:w="16838" w:h="11906"/></w:sectPr></w:pPr><w:r><w:t>six</w:t></w:r></w:p>
        <w:p><w:r><w:t>seven</w:t></w:r><w:r><w:br w:type="page"/></w:r></w:p>
        <w:p/>
        <w:sectPr><w:type w:val="continuous"/><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr>"#,
    );
    let shape: Vec<String> = imported
        .document
        .blocks
        .iter()
        .map(|block| match block {
            Block::PageBreak(_) => "|".to_owned(),
            Block::Paragraph(paragraph) => plain(paragraph),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        shape,
        [
            "", "one", "|", "two", "|", "three", "four", "|", "five", "six", "seven", ""
        ]
    );
    assert_eq!(imported.notices, [Notice::DifferentPageLayouts]);
    let layout = imported.document.page_layout.as_ref().unwrap();
    assert!((layout.margin_left_mm - 25.4).abs() < 0.001);
}

#[test]
fn drops_pictures_and_keeps_the_surrounding_text() {
    let picture = |id: &str, anchor: bool| {
        let (open, close) = if anchor {
            ("<wp:anchor>", "</wp:anchor>")
        } else {
            ("<wp:inline>", "</wp:inline>")
        };
        format!(
            r#"<w:r><w:drawing>{open}<wp:extent cx="952500" cy="476250"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="{id}"/></pic:blipFill></pic:pic></a:graphicData></a:graphic>{close}</w:drawing></w:r>"#
        )
    };
    let body = format!(
        "<w:p><w:r><w:t>before</w:t></w:r>{}{}<w:r><w:t> after</w:t></w:r></w:p>",
        picture("rId5", false),
        picture("rId6", true)
    );
    let rels = document_rels(
        r#"<Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/><Relationship Id="rId6" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="http://example.com/x.png" TargetMode="External"/>"#,
    );
    let document = document_xml(&body);
    let bytes = archive(&[
        ("[Content_Types].xml", CONTENT_TYPES.as_bytes()),
        ("_rels/.rels", ROOT_RELS.as_bytes()),
        ("word/document.xml", document.as_bytes()),
        ("word/_rels/document.xml.rels", rels.as_bytes()),
        ("word/media/image1.png", b"\x89PNG\r\n\x1a\n"),
    ]);
    let imported = run_import(&bytes);
    let found = paragraphs(&imported.document.blocks);
    assert_eq!(found.len(), 1);
    assert_eq!(plain(found[0]), "before after");
    assert_eq!(
        imported.notices,
        [Notice::DrawingsOmitted, Notice::FloatingFlattened]
    );
}

#[test]
fn text_boxes_are_flattened_after_their_anchor_paragraph() {
    let imported = import_body(
        r#"<w:p><w:r><w:t>anchor</w:t></w:r><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor><a:graphic><a:graphicData><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>boxed</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice><mc:Fallback><w:pict><w:txbxContent><w:p><w:r><w:t>boxed</w:t></w:r></w:p></w:txbxContent></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p>
        <w:p><w:r><w:t>after</w:t></w:r></w:p>"#,
    );
    let text: Vec<String> = paragraphs(&imported.document.blocks)
        .iter()
        .map(|paragraph| plain(paragraph))
        .collect();
    assert_eq!(text, ["anchor", "boxed", "after"]);
    assert_eq!(imported.notices, [Notice::FloatingFlattened]);
}

#[test]
fn headers_and_footers_frame_the_document_once() {
    let body = r#"<w:p><w:r><w:t>body</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rId2"/><w:footerReference w:type="default" r:id="rId3"/></w:sectPr>"#;
    let header = format!(
        r#"<w:hdr xmlns:w="{NS}"><w:p><w:fldSimple w:instr="MERGEFIELD تھانہ_نام"><w:r><w:t>x</w:t></w:r></w:fldSimple></w:p></w:hdr>"#
    );
    let footer =
        format!(r#"<w:ftr xmlns:w="{NS}"><w:p><w:r><w:t>footer</w:t></w:r></w:p></w:ftr>"#);
    let rels = document_rels(
        r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>"#,
    );
    let imported = run_import(&docx(
        body,
        &[
            ("word/header1.xml", &header),
            ("word/footer1.xml", &footer),
            ("word/_rels/document.xml.rels", &rels),
        ],
    ));
    let all = paragraphs(&imported.document.blocks);
    assert!(
        matches!(&all[0].children[0], Inline::Field(field) if field.reference == FieldReference::Unresolved("تھانہ نام".to_owned()))
    );
    assert_eq!(plain(all[1]), "body");
    assert_eq!(plain(all[2]), "footer");
    assert_eq!(imported.notices, [Notice::HeadersFootersAsContent]);
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

#[test]
fn suggests_the_document_title_as_a_name() {
    let core = r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>  ضمنی   رپورٹ </dc:title></cp:coreProperties>"#;
    let imported = run_import(&docx("<w:p/>", &[("docProps/core.xml", core)]));
    assert_eq!(imported.suggested_name.as_deref(), Some("ضمنی رپورٹ"));
}

#[test]
fn imports_are_sendable_to_and_from_a_background_thread() {
    let bytes = docx("<w:p><w:r><w:t>x</w:t></w:r></w:p>", &[]);
    let handle = std::thread::spawn(move || {
        import(
            &bytes,
            &FieldCatalog::new(Vec::new()),
            &FieldMarkers::default(),
        )
    });
    let imported = handle.join().unwrap().unwrap();
    assert!(imported.notices.is_empty());
}
