#![allow(dead_code)]

use std::rc::Rc;

use missal_core::document::{
    Block, Document, ElementProps, Field, Inline, PageLayout, Paragraph, Style, Table, TableCell,
    TableRow, Text, TextFormat,
};
use missal_core::{
    FieldCatalog, FieldMarkers, FieldReference, FirField, FirProperty, Placeholder, PlaceholderId,
    PlaceholderSource,
};
use missal_editor::model::selection::pos;
use missal_editor::{EditorCore, Selection};
use missal_page::{BlockPath, Position};

pub fn catalog() -> Rc<FieldCatalog> {
    Rc::new(FieldCatalog::new(vec![
        Placeholder {
            id: PlaceholderId::new(1).unwrap(),
            label: "تھانہ".into(),
            source: PlaceholderSource::Shared,
        },
        Placeholder {
            id: PlaceholderId::new(2).unwrap(),
            label: "بینک".into(),
            source: PlaceholderSource::Custom,
        },
    ]))
}

pub fn fir_no() -> FieldReference {
    FieldReference::Fir(FirField {
        property: FirProperty::FirNo,
        index: None,
    })
}

pub fn offence() -> FieldReference {
    FieldReference::Fir(FirField {
        property: FirProperty::Offence,
        index: None,
    })
}

pub fn text(s: &str) -> Inline {
    Inline::Text(Text::new(s))
}

pub fn styled(s: &str, css: &str, format: u32) -> Inline {
    Inline::Text(Text {
        text: s.into(),
        format: TextFormat(format),
        style: Style::non_empty(css),
        ..Text::default()
    })
}

pub fn field(reference: FieldReference) -> Inline {
    Inline::Field(Field::new(reference))
}

pub fn para(children: Vec<Inline>) -> Block {
    Block::Paragraph(Paragraph::new(children))
}

pub fn p(s: &str) -> Block {
    if s.is_empty() {
        para(Vec::new())
    } else {
        para(vec![text(s)])
    }
}

pub fn cell(blocks: Vec<Block>) -> TableCell {
    TableCell {
        props: ElementProps::rtl(),
        blocks,
        ..TableCell::default()
    }
}

pub fn table(rows: Vec<Vec<TableCell>>) -> Block {
    Block::Table(Table {
        props: ElementProps::rtl(),
        rows: rows
            .into_iter()
            .map(|cells| TableRow {
                props: ElementProps::rtl(),
                cells,
                ..TableRow::default()
            })
            .collect(),
        ..Table::default()
    })
}

pub fn grid(rows: usize, cols: usize) -> Block {
    table(
        (0..rows)
            .map(|r| {
                (0..cols)
                    .map(|c| cell(vec![p(&format!("خانہ {r}{c}"))]))
                    .collect()
            })
            .collect(),
    )
}

pub fn doc(blocks: Vec<Block>) -> Document {
    Document::new(blocks)
}

pub fn small_page(mut document: Document) -> Document {
    document.page_layout = PageLayout::new(120.0, 90.0, [10.0; 4]);
    document
}

pub fn core(document: Document) -> EditorCore {
    EditorCore::new(document, catalog(), FieldMarkers::default())
}

pub fn at(steps: &[u32], offset: usize) -> Position {
    pos(BlockPath::new(steps), offset)
}

pub fn caret(core: &mut EditorCore, steps: &[u32], offset: usize) {
    core.set_selection(Selection::caret(at(steps, offset)));
}

pub fn select(core: &mut EditorCore, a: (&[u32], usize), b: (&[u32], usize)) {
    core.set_selection(Selection::new(at(a.0, a.1), at(b.0, b.1)));
}

pub fn block_text(core: &EditorCore, steps: &[u32]) -> String {
    core.block_text(&BlockPath::new(steps))
        .unwrap_or_else(|| panic!("no text block at {steps:?}"))
}

pub fn texts(core: &EditorCore) -> Vec<String> {
    missal_editor::model::tree::all_text_paths(core.document())
        .iter()
        .map(|path| core.block_text(path).unwrap())
        .collect()
}

pub fn head(core: &EditorCore) -> (Vec<u32>, usize) {
    let head = &core.selection().head;
    (head.path.steps().to_vec(), head.offset)
}

pub fn bundle() -> Vec<(String, Document)> {
    missal_store::TemplateBundle::embedded()
        .templates()
        .unwrap()
}

pub fn largest_template() -> (String, Document) {
    bundle()
        .into_iter()
        .max_by_key(|(_, d)| d.encode().len())
        .unwrap()
}

pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }

    pub fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}
