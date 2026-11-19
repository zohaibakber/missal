use missal_core::document::{
    Block, Document, ElementProps, Inline, Leaf, PageLayout, Paragraph, Table, TableCell, TableRow,
    Text,
};
use missal_page::{
    Affinity, BlockPath, DocSource, Edit, LayoutCache, Packet, PageEngine, Point, Position,
    TokenFields,
};

const WORDS: &str = "مدعی مقدمہ نے تھانہ صدر میں درخواست دی کہ مورخہ بارہ مارچ کی رات نامعلوم ملزمان اس کے گھر میں داخل ہوئے اور قیمتی سامان چوری کر کے فرار ہو گئے";

fn words(n: usize) -> String {
    WORDS
        .split(' ')
        .cycle()
        .take(n)
        .collect::<Vec<_>>()
        .join(" ")
}

fn para(text: &str) -> Block {
    Block::Paragraph(Paragraph::new(vec![Inline::Text(Text::new(text))]))
}

fn page_break() -> Block {
    Block::PageBreak(Leaf::default())
}

fn cell(blocks: Vec<Block>) -> TableCell {
    TableCell {
        props: ElementProps::rtl(),
        blocks,
        ..TableCell::default()
    }
}

fn table(rows: Vec<Vec<TableCell>>) -> Block {
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

fn small(blocks: Vec<Block>) -> Document {
    let mut doc = Document::new(blocks);
    doc.page_layout = PageLayout::new(120.0, 90.0, [10.0; 4]);
    doc
}

fn layout(doc: &Document) -> (PageEngine, Packet) {
    let mut engine = PageEngine::new();
    let packet = engine.layout(&[DocSource::new(doc, &TokenFields)]);
    (engine, packet)
}

fn boundaries(text: &str) -> Vec<usize> {
    let mut out: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    out.push(text.len());
    out
}

fn assert_round_trip(packet: &Packet, path: &BlockPath, text: &str) -> usize {
    let mut checked = 0;
    for offset in boundaries(text) {
        for affinity in [Affinity::Downstream, Affinity::Upstream] {
            let position = Position::new(0, path.clone(), offset).with_affinity(affinity);
            let caret = packet.caret(&position).expect("caret");
            let probe = Point::new(caret.rect.x, caret.rect.y + caret.rect.height / 2.0);
            let hit = packet.hit_test(caret.page, probe).expect("hit");
            assert_eq!(hit.path, *path, "offset {offset}");
            let back = packet.caret(&hit).expect("caret of hit");
            assert_eq!(back.page, caret.page, "offset {offset}");
            assert!(
                (back.rect.x - caret.rect.x).abs() < 1.0
                    && (back.rect.y - caret.rect.y).abs() < 1.0,
                "offset {offset} {affinity:?}: {:?} -> {hit:?} -> {:?}",
                caret.rect,
                back.rect
            );
            checked += 1;
        }
    }
    checked
}

#[test]
fn caret_and_hit_test_round_trip_in_paragraphs() {
    let text = words(60);
    let doc = small(vec![
        para(&text),
        para("Section 380 PPC کے تحت مقدمہ 412/2026 درج ہوا"),
    ]);
    let (_, packet) = layout(&doc);
    assert!(
        packet
            .lines(0)
            .iter()
            .filter(|l| l.path == BlockPath::top(0))
            .count()
            > 2
    );
    assert!(assert_round_trip(&packet, &BlockPath::top(0), &text) > 100);
    assert_round_trip(
        &packet,
        &BlockPath::top(1),
        "Section 380 PPC کے تحت مقدمہ 412/2026 درج ہوا",
    );
}

#[test]
fn row_taller_than_a_page_makes_progress() {
    let tall = words(500);
    let (_, packet) = layout(&small(vec![table(vec![vec![
        cell(vec![para(&tall)]),
        cell(vec![para("الف")]),
    ]])]));
    assert!(packet.page_count() >= 3);
    let lines = packet.lines(0);
    let total = lines
        .iter()
        .filter(|l| l.path == BlockPath::new(&[0, 0, 0, 0]))
        .count();
    let pages: std::collections::BTreeSet<usize> = lines.iter().map(|l| l.page).collect();
    assert!(total > 20 && pages.len() >= 3);
}

type Signature = Vec<(Vec<u32>, usize, usize, i32, std::ops::Range<usize>)>;

fn line_signature(packet: &Packet) -> Signature {
    packet
        .lines(0)
        .into_iter()
        .map(|l| {
            (
                l.path.0,
                l.line,
                l.page,
                (l.rect.y * 10.0).round() as i32,
                l.text,
            )
        })
        .collect()
}

#[test]
fn incremental_relayout_matches_full_layout() {
    let mut blocks = Vec::new();
    for i in 0..60 {
        if i % 7 == 3 {
            blocks.push(table(vec![vec![
                cell(vec![para(&words(9))]),
                cell(vec![para(&words(4))]),
            ]]));
        } else if i % 13 == 5 {
            blocks.push(page_break());
        } else {
            blocks.push(para(&words(10 + (i * 7) % 40)));
        }
    }
    let mut doc = small(blocks);
    let mut engine = PageEngine::new();
    let mut packet = engine.layout(&[DocSource::new(&doc, &TokenFields)]);
    type Apply = Box<dyn Fn(&mut Document)>;
    let edits: Vec<(Edit, Apply)> = vec![
        (
            Edit::Block(BlockPath::top(1)),
            Box::new(|d: &mut Document| d.blocks[1] = para(&words(3))),
        ),
        (
            Edit::Block(BlockPath::top(20)),
            Box::new(|d: &mut Document| d.blocks[20] = para(&words(90))),
        ),
        (
            Edit::Block(BlockPath::new(&[3, 0, 0, 0])),
            Box::new(|d: &mut Document| {
                if let Block::Table(t) = &mut d.blocks[3]
                    && let row = &mut t.rows[0]
                    && let c = &mut row.cells[0]
                {
                    c.blocks[0] = para(&words(60));
                }
            }),
        ),
        (
            Edit::Splice {
                start: 10,
                removed: 0,
                inserted: 2,
            },
            Box::new(|d: &mut Document| {
                d.blocks.insert(10, para(&words(30)));
                d.blocks.insert(10, page_break());
            }),
        ),
        (
            Edit::Splice {
                start: 30,
                removed: 3,
                inserted: 0,
            },
            Box::new(|d: &mut Document| {
                d.blocks.drain(30..33);
            }),
        ),
        (
            Edit::Block(BlockPath::top(55)),
            Box::new(|d: &mut Document| d.blocks[55] = para("ختم")),
        ),
    ];
    for (edit, apply) in edits {
        apply(&mut doc);
        engine.update(&mut packet, 0, &DocSource::new(&doc, &TokenFields), &edit);
        let mut fresh_engine = PageEngine::with_cache(LayoutCache::default());
        let fresh = fresh_engine.layout(&[DocSource::new(&doc, &TokenFields)]);
        assert_eq!(packet.page_count(), fresh.page_count(), "{edit:?}");
        assert_eq!(line_signature(&packet), line_signature(&fresh), "{edit:?}");
    }
}
