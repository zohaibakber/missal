use std::path::Path;

use missal_core::document::{
    Align, Block, Direction, Document, ElementProps, Field, Heading, Inline, Leaf, Link, List,
    ListItem, ListItemChild, ListTag, ListType, PageLayout, Paragraph, Quote, Style, Table,
    TableCell, TableRow, Text, TextFormat,
};
use missal_core::field::FirField;
use missal_core::{
    FieldCatalog, FieldMarkers, FieldReference, FirProperty, Placeholder, PlaceholderId,
    PlaceholderSource,
};
use missal_docx::{export, import};

fn template_catalog(root: &Path) -> FieldCatalog {
    let fields: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("fields.json")).unwrap()).unwrap();
    let labels = ["global", "custom"].into_iter().flat_map(|key| {
        fields[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|label| label.as_str().unwrap().trim().to_owned())
            .collect::<Vec<_>>()
    });
    FieldCatalog::new(
        labels
            .enumerate()
            .map(|(index, label)| Placeholder {
                id: PlaceholderId::new(index as i64 + 1).unwrap(),
                label,
                source: PlaceholderSource::Custom,
            })
            .collect(),
    )
}

#[derive(Debug, PartialEq)]
enum Token {
    Number(f64),
    Text(String),
}

fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = text.char_indices().peekable();
    let mut word = String::new();
    while let Some((start, c)) = chars.next() {
        let starts_number = c.is_ascii_digit()
            || (c == '-' && chars.peek().is_some_and(|(_, next)| next.is_ascii_digit()));
        if !starts_number {
            word.push(c);
            continue;
        }
        if !word.is_empty() {
            out.push(Token::Text(std::mem::take(&mut word)));
        }
        let mut end = start + c.len_utf8();
        while let Some((index, next)) = chars.peek().copied() {
            if next.is_ascii_digit() || next == '.' {
                end = index + next.len_utf8();
                chars.next();
            } else {
                break;
            }
        }
        out.push(Token::Number(text[start..end].parse().unwrap_or(f64::NAN)));
    }
    if !word.is_empty() {
        out.push(Token::Text(word));
    }
    out
}

fn normalized(document: &Document) -> String {
    format!("{document:?}").replace(r#"font-family: \"Jameel Noori Nastaleeq\"; "#, "")
}

fn assert_round_trip(name: &str, before: &Document, after: &Document) {
    let (left, right) = (normalized(before), normalized(after));
    let (left_tokens, right_tokens) = (tokens(&left), tokens(&right));
    let mismatch = left_tokens
        .iter()
        .zip(&right_tokens)
        .position(|pair| match pair {
            (Token::Number(a), Token::Number(b)) => (a - b).abs() > 0.11,
            (a, b) => a != b,
        });
    if mismatch.is_none() && left_tokens.len() == right_tokens.len() {
        return;
    }
    let at = mismatch.unwrap_or(left_tokens.len().min(right_tokens.len()));
    let context = |tokens: &[Token]| {
        tokens[at.saturating_sub(30)..(at + 12).min(tokens.len())]
            .iter()
            .map(|token| match token {
                Token::Number(number) => number.to_string(),
                Token::Text(text) => text.clone(),
            })
            .collect::<String>()
    };
    panic!(
        "{name} changed on a round trip\nbefore: {}\nafter:  {}",
        context(&left_tokens),
        context(&right_tokens)
    );
}

#[test]
fn every_bundled_template_survives_export_and_reimport() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates");
    let catalog = template_catalog(&root);
    let markers = FieldMarkers::default();
    let mut files: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "docx")
        })
        .collect();
    files.sort();
    assert!(!files.is_empty());
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let original = import(&std::fs::read(&path).unwrap(), &catalog, &markers).unwrap();
        let bytes = export(&original.document, &catalog, &markers, Some(&name)).unwrap();
        let again = import(&bytes, &catalog, &markers).unwrap();
        assert!(again.notices.is_empty(), "{name}: {:?}", again.notices);
        assert_eq!(again.suggested_name.as_deref(), Some(name.as_str()));
        assert_round_trip(&name, &original.document, &again.document);
    }
}

fn text(content: &str) -> Inline {
    Inline::Text(Text::new(content))
}

fn styled(content: &str, format: TextFormat, css: &str) -> Inline {
    Inline::Text(Text {
        text: content.to_owned(),
        format,
        style: Style::non_empty(css),
        imported_style: None,
    })
}

fn paragraph(direction: Direction, children: Vec<Inline>) -> Block {
    Block::Paragraph(Paragraph {
        props: ElementProps {
            direction: Some(direction),
            ..ElementProps::default()
        },
        children,
    })
}

fn cell(content: &str) -> TableCell {
    TableCell {
        blocks: vec![paragraph(Direction::Rtl, vec![text(content)])],
        ..TableCell::default()
    }
}

fn item(children: Vec<ListItemChild>) -> ListItem {
    ListItem {
        children,
        ..ListItem::default()
    }
}

fn list(list_type: ListType, start: i64, items: Vec<ListItem>) -> List {
    List {
        list_type: Some(list_type),
        tag: Some(if list_type == ListType::Number {
            ListTag::Ol
        } else {
            ListTag::Ul
        }),
        start: Some(start),
        items,
        ..List::default()
    }
}

fn authored() -> (Document, FieldCatalog) {
    let station = PlaceholderId::new(7).unwrap();
    let catalog = FieldCatalog::new(vec![Placeholder {
        id: station,
        label: "تھانہ نام".to_owned(),
        source: PlaceholderSource::Shared,
    }]);
    let accused = FieldReference::Fir(FirField {
        property: FirProperty::Accused,
        index: Some(1),
    });
    let blocks = vec![
        Block::Heading(Heading {
            props: ElementProps::rtl(),
            level: Some(missal_core::document::HeadingLevel::H1),
            children: vec![text("رپورٹ")],
        }),
        paragraph(
            Direction::Rtl,
            vec![
                text("ملزم "),
                Inline::Field(Field::new(accused)),
                text(" بمقام "),
                Inline::Field(Field::new(FieldReference::Catalog(station))),
                Inline::LineBreak(Leaf::default()),
                Inline::Field(Field::new(FieldReference::Unresolved(
                    "نامعلوم خانہ".to_owned(),
                ))),
            ],
        ),
        paragraph(
            Direction::Ltr,
            vec![
                styled("Bold", TextFormat::BOLD, ""),
                styled(" italic", TextFormat::ITALIC, "color: #c00000"),
                styled(" under", TextFormat::UNDERLINE, "font-size: 24px"),
                styled(" struck", TextFormat::STRIKETHROUGH, ""),
                styled("2", TextFormat::SUPERSCRIPT, ""),
                styled(" marked", TextFormat::HIGHLIGHT, ""),
                styled(" caps", TextFormat::UPPERCASE, ""),
                Inline::Link(Link {
                    url: Some("https://example.com/a?b=1&c=2".to_owned()),
                    children: vec![text(" link")],
                    ..Link::default()
                }),
            ],
        ),
        Block::Quote(Quote {
            props: ElementProps::rtl(),
            children: vec![text("اقتباس")],
        }),
        Block::List(list(
            ListType::Number,
            3,
            vec![
                item(vec![ListItemChild::Inline(text("پہلا"))]),
                item(vec![
                    ListItemChild::Inline(text("دوسرا")),
                    ListItemChild::List(list(
                        ListType::Bullet,
                        1,
                        vec![item(vec![ListItemChild::Inline(text("ذیلی"))])],
                    )),
                ]),
            ],
        )),
        Block::List(List {
            items: vec![
                ListItem {
                    checked: Some(true),
                    ..item(vec![ListItemChild::Inline(text("ہو گیا"))])
                },
                item(vec![ListItemChild::Inline(text("باقی"))]),
            ],
            ..list(ListType::Check, 1, Vec::new())
        }),
        Block::PageBreak(Leaf::default()),
        Block::Table(Table {
            props: ElementProps::rtl(),
            col_widths: Some(vec![120.0, 200.0, 160.0]),
            rows: vec![
                TableRow {
                    cells: vec![
                        TableCell {
                            col_span: Some(2),
                            header_state: Some(1),
                            background_color: Some("#d9e2f3".to_owned()),
                            ..cell("سرخی")
                        },
                        TableCell {
                            row_span: Some(2),
                            header_state: Some(1),
                            vertical_align: Some("middle".to_owned()),
                            ..cell("دائیں")
                        },
                    ],
                    ..TableRow::default()
                },
                TableRow {
                    cells: vec![cell("الف"), cell("ب")],
                    ..TableRow::default()
                },
            ],
            ..Table::default()
        }),
        Block::HorizontalRule(Leaf::default()),
        paragraph(Direction::Rtl, vec![text("آخر")]),
    ];
    let mut document = Document::new(blocks);
    document.page_layout = PageLayout::new(297.0, 210.0, [12.7, 15.0, 12.7, 20.0]);
    (document, catalog)
}

fn inline_text(inlines: &[Inline], catalog: &FieldCatalog, out: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Text(text) => out.push_str(&text.text),
            Inline::LineBreak(_) => out.push('\n'),
            Inline::Field(field) => {
                out.push('[');
                out.push_str(&catalog.label(&field.reference).unwrap_or_default());
                out.push(']');
            }
            Inline::Link(link) => inline_text(&link.children, catalog, out),
        }
    }
}

fn texts_of(paragraph: &Paragraph) -> Vec<&Text> {
    let mut found = Vec::new();
    for inline in &paragraph.children {
        match inline {
            Inline::Text(text) => found.push(text),
            Inline::Link(link) => {
                found.extend(link.children.iter().filter_map(|child| match child {
                    Inline::Text(text) => Some(text),
                    _ => None,
                }))
            }
            _ => {}
        }
    }
    found
}

fn text_named<'d>(paragraph: &'d Paragraph, needle: &str) -> &'d Text {
    texts_of(paragraph)
        .into_iter()
        .find(|text| text.text.contains(needle))
        .unwrap_or_else(|| panic!("no run contains {needle:?}"))
}

fn declaration<'s>(style: &'s Option<Style>, property: &str) -> Option<&'s str> {
    style.as_ref().and_then(|style| style.get(property))
}

#[test]
fn an_authored_document_keeps_its_content_direction_fields_tables_and_lists_in_word() {
    let (document, catalog) = authored();
    let markers = FieldMarkers::default();
    let bytes = export(&document, &catalog, &markers, Some("رپورٹ")).unwrap();
    let imported = import(&bytes, &catalog, &markers).unwrap();
    assert!(imported.notices.is_empty(), "{:?}", imported.notices);
    assert_eq!(imported.suggested_name.as_deref(), Some("رپورٹ"));
    let blocks = &imported.document.blocks;

    let layout = imported.document.page_layout.as_ref().unwrap();
    assert!((layout.width_mm - 297.0).abs() < 0.05);
    assert!((layout.height_mm - 210.0).abs() < 0.05);
    assert!((layout.margin_left_mm - 20.0).abs() < 0.05);
    assert!((layout.margin_right_mm - 15.0).abs() < 0.05);

    let Block::Paragraph(heading) = &blocks[0] else {
        panic!("heading should arrive as a paragraph, got {:?}", blocks[0]);
    };
    let title = text_named(heading, "رپورٹ");
    assert!(title.format.contains(TextFormat::BOLD));
    assert_eq!(declaration(&title.style, "font-size"), Some("40px"));
    assert_eq!(heading.props.direction, Some(Direction::Rtl));

    let Block::Paragraph(fields) = &blocks[1] else {
        panic!("expected the field paragraph");
    };
    let mut shown = String::new();
    inline_text(&fields.children, &catalog, &mut shown);
    assert_eq!(
        shown,
        "ملزم [نام ملزم و سکونت 1] بمقام [تھانہ نام]\n[نامعلوم خانہ]"
    );
    let references: Vec<&FieldReference> = fields
        .children
        .iter()
        .filter_map(|inline| match inline {
            Inline::Field(field) => Some(&field.reference),
            _ => None,
        })
        .collect();
    assert_eq!(
        references,
        [
            &FieldReference::Fir(FirField {
                property: FirProperty::Accused,
                index: Some(1)
            }),
            &FieldReference::Catalog(PlaceholderId::new(7).unwrap()),
            &FieldReference::Unresolved("نامعلوم خانہ".to_owned()),
        ]
    );

    let Block::Paragraph(latin) = &blocks[2] else {
        panic!("expected the formatted paragraph");
    };
    assert_eq!(latin.props.direction, Some(Direction::Ltr));
    let format_of = |needle: &str| text_named(latin, needle).format;
    assert!(format_of("Bold").contains(TextFormat::BOLD));
    assert!(format_of("italic").contains(TextFormat::ITALIC));
    assert!(format_of("under").contains(TextFormat::UNDERLINE));
    assert!(format_of("struck").contains(TextFormat::STRIKETHROUGH));
    assert!(format_of("2").contains(TextFormat::SUPERSCRIPT));
    assert!(format_of("caps").contains(TextFormat::UPPERCASE));
    assert_eq!(
        declaration(&text_named(latin, "italic").style, "color"),
        Some("rgb(192, 0, 0)")
    );
    assert_eq!(
        declaration(&text_named(latin, "under").style, "font-size"),
        Some("24px")
    );
    assert_eq!(
        declaration(&text_named(latin, "marked").style, "background-color"),
        Some("rgb(255, 255, 0)")
    );
    let link = latin.children.iter().find_map(|inline| match inline {
        Inline::Link(link) => link.url.as_deref(),
        _ => None,
    });
    assert_eq!(link, Some("https://example.com/a?b=1&c=2"));

    let Block::Paragraph(quote) = &blocks[3] else {
        panic!("expected the quote");
    };
    assert!(declaration(&quote.props.imported_style, "border-style").is_some());

    let Block::List(numbered) = &blocks[4] else {
        panic!("expected the numbered list, got {:?}", blocks[4]);
    };
    assert_eq!(numbered.list_type, Some(ListType::Number));
    assert_eq!(numbered.start, Some(3));
    let values: Vec<Option<i64>> = numbered.items.iter().map(|item| item.value).collect();
    assert_eq!(values[..2], [Some(3), Some(4)]);
    let nested = numbered
        .items
        .iter()
        .find_map(|item| {
            item.children.iter().find_map(|child| match child {
                ListItemChild::List(list) => Some(list),
                ListItemChild::Inline(_) => None,
            })
        })
        .expect("the bullet list stays nested");
    assert_eq!(nested.list_type, Some(ListType::Bullet));

    let Block::List(checks) = &blocks[5] else {
        panic!("expected the checklist, got {:?}", blocks[5]);
    };
    assert_eq!(checks.items.len(), 2);
    assert!(matches!(blocks[6], Block::PageBreak(_)));

    let Block::Table(table) = &blocks[7] else {
        panic!("expected the table, got {:?}", blocks[7]);
    };
    assert_eq!(table.props.direction, Some(Direction::Rtl));
    assert_eq!(
        table.col_widths.as_deref(),
        Some(&[120.0, 200.0, 160.0][..])
    );
    assert_eq!(table.rows.len(), 2);
    let top = &table.rows[0].cells;
    assert_eq!(top[0].col_span, Some(2));
    assert_eq!(
        top[0].background_color.as_deref(),
        Some("rgb(217, 226, 243)")
    );
    assert_eq!(top[1].row_span, Some(2));
    assert_eq!(top[1].vertical_align.as_deref(), Some("middle"));
    assert_eq!(table.rows[1].cells.len(), 2);
    let cell_text = |cell: &TableCell| match &cell.blocks[0] {
        Block::Paragraph(paragraph) => {
            let mut out = String::new();
            inline_text(&paragraph.children, &catalog, &mut out);
            out
        }
        other => panic!("cell holds {other:?}"),
    };
    assert_eq!(cell_text(&top[0]), "سرخی");
    assert_eq!(cell_text(&table.rows[1].cells[1]), "ب");
    assert!(declaration(&top[0].props.imported_style, "border-width").is_some());

    let Block::Paragraph(rule) = &blocks[8] else {
        panic!("expected the rule");
    };
    assert!(declaration(&rule.props.imported_style, "border-style").is_some());
    let Block::Paragraph(last) = &blocks[9] else {
        panic!("expected the closing paragraph");
    };
    assert_eq!(texts_of(last)[0].text, "آخر");
    assert_eq!(last.props.align, Some(Align::Start));
}
