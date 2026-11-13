use std::collections::HashMap;

use missal_core::document::{
    Align, Block, Direction, Envelope, Field, Inline, Paragraph, Style, Text, TextFormat,
    project_state,
};
use missal_core::field::FirField;
use missal_core::{Document, FieldReference, FirProperty, PlaceholderId};
use serde_json::{Value, json};

fn envelope(children: Value) -> Envelope {
    Envelope::from_value(json!({
        "format": "missal-lexical",
        "version": 1,
        "extraEnvelopeKey": [1, 2],
        "state": {
            "root": {"type": "root", "direction": null, "format": "", "indent": 0, "children": children, "custom": true},
            "selection": {"anchor": 1}
        }
    }))
    .unwrap()
}

fn round_trip(children: Value) -> (Envelope, Document) {
    let envelope = envelope(children);
    let document = Document::from_envelope(&envelope).unwrap();
    let encoded = document.to_envelope();
    assert_eq!(encoded.expanded_state(), envelope.expanded_state());
    assert_eq!(encoded.to_value()["extraEnvelopeKey"], json!([1, 2]));
    assert_eq!(Document::decode(&document.encode()).unwrap(), document);
    assert_eq!(document.projections(), project_state(&envelope.state));
    (envelope, document)
}

#[test]
fn unknown_nodes_and_keys_survive() {
    let (_, document) = round_trip(json!([
        {"type": "paragraph", "direction": null, "format": "bogus", "indent": 2, "children": [
            {"type": "text", "text": "a ", "format": 9, "style": "color: red", "detail": 0, "mode": "token", "version": 1, "unknown": {"x": 1}},
            {"type": "sparkle", "children": [{"type": "text", "text": "nested"}], "glitter": 3},
            {"type": "text", "text": 5},
            {"type": "linebreak", "$": {"importedStyle": "font-size: 9pt", "other": 1}},
            {"type": "field", "reference": {"_tag": "FirField", "property": "fir_no", "extra": true}},
            {"type": "field", "reference": {"_tag": "CatalogField", "id": 0}},
            {"type": "field"},
            {"type": "field", "reference": {"_tag": "UnresolvedToken", "text": "بینک"}, "children": [{"type": "text", "text": "odd"}]},
            {"type": "image", "src": "data:image/png;base64,AA", "width": null, "height": 20.5},
            {"type": "link", "url": "https://example.com", "rel": null, "children": [{"type": "text", "text": "link"}]}
        ]},
        {"type": "heading", "tag": "h2", "children": [{"type": "text", "text": "عنوان"}]},
        {"type": "quote", "children": []},
        {"type": "list", "listType": "number", "start": 3, "tag": "ol", "children": [
            {"type": "listitem", "value": 3, "children": [{"type": "text", "text": "one"}]},
            {"type": "listitem", "value": 4, "children": [{"type": "list", "listType": "bullet", "tag": "ul", "children": [
                {"type": "listitem", "checked": true, "children": [{"type": "text", "text": "two"}]}
            ]}]},
            {"type": "paragraph", "children": []}
        ]},
        {"type": "table", "colWidths": [100, 50.5], "children": [
            {"type": "tablerow", "height": 12, "children": [
                {"type": "tablecell", "colSpan": 2, "headerState": 1, "backgroundColor": null, "width": 75, "children": [
                    {"type": "paragraph", "children": [{"type": "text", "text": "cell"}]}
                ]},
                {"type": "text", "text": "stray"}
            ]},
            "not a node"
        ]},
        {"type": "pagebreak"},
        {"type": "horizontalrule", "version": 1},
        {"type": "text", "text": "stray at root"},
        null
    ]));
    let Block::Paragraph(paragraph) = &document.blocks[0] else {
        panic!("expected paragraph");
    };
    assert_eq!(paragraph.props.direction, None);
    assert_eq!(paragraph.props.align, None);
    assert_eq!(paragraph.props.indent, Some(2));
    let Inline::Text(text) = &paragraph.children[0] else {
        panic!("expected text");
    };
    assert!(text.format().contains(TextFormat::BOLD));
    assert!(text.format().contains(TextFormat::UNDERLINE));
    assert_eq!(text.css().map(Style::as_str), Some("color: red"));
    assert!(matches!(paragraph.children[1], Inline::Other(_)));
    assert!(matches!(paragraph.children[2], Inline::Other(_)));
    let Inline::Field(field) = &paragraph.children[4] else {
        panic!("expected field");
    };
    assert_eq!(
        field.reference,
        FieldReference::Fir(FirField {
            property: FirProperty::FirNo,
            index: None
        })
    );
    let projections = document.projections();
    assert_eq!(
        projections.plain_text,
        "a nested oddlinkعنوانonetwocellstraystray at root"
    );
    assert_eq!(projections.field_count, 2);
}

#[test]
fn bind_replaces_raw_and_typed_references() {
    let (_, mut document) = round_trip(json!([
        {"type": "paragraph", "children": [
            {"type": "field", "reference": {"_tag": "UnresolvedToken", "text": "Date_FIR"}},
            {"type": "field", "reference": {"_tag": "UnresolvedToken", "text": " بینک ", "note": 1}},
            {"type": "field", "reference": {"_tag": "UnresolvedToken", "text": "unknown"}},
            {"type": "mystery", "reference": {"_tag": "UnresolvedToken", "text": "بینک"}}
        ]}
    ]));
    let ids = HashMap::from([("بینک".to_owned(), PlaceholderId::new(5).unwrap())]);
    document.bind_catalog_fields(&ids);
    let references = document.field_references();
    assert_eq!(
        references,
        [
            FieldReference::Fir(FirField {
                property: FirProperty::Date,
                index: None
            }),
            FieldReference::Catalog(PlaceholderId::new(5).unwrap()),
            FieldReference::Unresolved("unknown".into()),
        ]
    );
    let state = document.to_envelope().expanded_state();
    assert_eq!(
        state["root"]["children"][0]["children"][3]["reference"],
        json!({"_tag": "CatalogField", "id": 5})
    );
    document.unbind_catalog_fields(&|id| (id.get() == 5).then(|| "بینک".to_owned()));
    let state = document.to_envelope().expanded_state();
    assert_eq!(
        state["root"]["children"][0]["children"][1]["reference"],
        json!({"_tag": "UnresolvedToken", "text": "بینک"})
    );
}

#[test]
fn editing_the_typed_tree_writes_through() {
    let mut document = Document::empty();
    let Block::Paragraph(paragraph) = &mut document.blocks[0] else {
        panic!("empty document has a paragraph");
    };
    paragraph.props.align = Some(Align::Center);
    let mut bold = Text::new("سلام ");
    bold.format = Some(TextFormat::BOLD);
    bold.style = Some(Style::new("font-size: 16px"));
    paragraph.children.push(Inline::Text(bold));
    paragraph
        .children
        .push(Inline::Field(Field::new(FieldReference::Fir(FirField {
            property: FirProperty::Offence,
            index: None,
        }))));
    document
        .blocks
        .push(Block::Paragraph(Paragraph::new(vec![Inline::Text(
            Text::new("دوسرا"),
        )])));
    let envelope = document.to_envelope();
    assert_eq!(envelope.styles, ["font-size: 16px"]);
    let state = &envelope.state["root"]["children"];
    assert_eq!(state[0]["format"], "center");
    assert_eq!(state[0]["children"][0]["style"], 0);
    assert_eq!(state[1]["direction"], "rtl");
    let decoded = Document::decode(&document.encode()).unwrap();
    assert_eq!(decoded.blocks[1], document.blocks[1]);
    assert_eq!(Document::decode(&decoded.encode()).unwrap(), decoded);
    let Block::Paragraph(first) = &decoded.blocks[0] else {
        panic!("paragraph survives");
    };
    assert_eq!(first.props.align, Some(Align::Center));
    assert!(!first.props.extra.contains_key("format"));
    assert_eq!(decoded.projections().plain_text, "سلام دوسرا");
    assert_eq!(decoded.root.direction, Some(Direction::Rtl));
}

#[test]
fn style_declarations_are_parsed() {
    let style = Style::new(
        "font-family: 'Jameel Noori Nastaleeq'; font-size:16px ;color: red; COLOR: blue",
    );
    assert_eq!(style.get("font-size"), Some("16px"));
    assert_eq!(style.get("color"), Some("blue"));
    assert_eq!(style.declarations().count(), 4);
}
