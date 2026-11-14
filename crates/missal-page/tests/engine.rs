use missal_core::document::{
    Block, Document, ElementProps, Field, Inline, Leaf, PageLayout, Paragraph, Style, Table,
    TableCell, TableRow, Text,
};
use missal_core::{FieldDisplayText, FieldReference};
use missal_page::{
    Affinity, BlockPath, DocSource, Edit, FieldSource, Packet, PageEngine, Point, Position,
    TokenFields, VerticalDirection, logical_text,
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

fn styled_para(text: &str, css: &str) -> Block {
    let mut p = Paragraph::new(vec![Inline::Text(Text::new(text))]);
    p.props.imported_style = Some(Style::new(css));
    Block::Paragraph(p)
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

fn header_cell(blocks: Vec<Block>) -> TableCell {
    TableCell {
        props: ElementProps::rtl(),
        header_state: Some(1),
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
fn caret_and_hit_test_round_trip_in_table_cells() {
    let a = words(20);
    let b = words(7);
    let doc = small(vec![table(vec![
        vec![cell(vec![para(&a)]), cell(vec![para(&b), para("دوسرا")])],
        vec![cell(vec![para("نیچے")]), cell(vec![para(&b)])],
    ])]);
    let (_, packet) = layout(&doc);
    assert_round_trip(&packet, &BlockPath::new(&[0, 0, 0, 0]), &a);
    assert_round_trip(&packet, &BlockPath::new(&[0, 0, 1, 0]), &b);
    assert_round_trip(&packet, &BlockPath::new(&[0, 0, 1, 1]), "دوسرا");
    assert_round_trip(&packet, &BlockPath::new(&[0, 1, 1, 0]), &b);
    let first = packet
        .caret(&Position::new(0, BlockPath::new(&[0, 0, 0, 0]), 0))
        .unwrap();
    let second = packet
        .caret(&Position::new(0, BlockPath::new(&[0, 0, 1, 0]), 0))
        .unwrap();
    assert!(
        first.rect.x > second.rect.x,
        "rtl table: first cell is on the right"
    );
}

#[test]
fn mapping_works_across_pages() {
    let text = words(400);
    let doc = small(vec![para(&text)]);
    let (_, packet) = layout(&doc);
    assert!(packet.page_count() >= 3);
    let lines = packet.lines(0);
    let on_later_page = lines.iter().find(|l| l.page == 2).expect("line on page 3");
    let position = Position::new(0, BlockPath::top(0), on_later_page.text.start + 4);
    let caret = packet.caret(&position).unwrap();
    assert_eq!(caret.page, 2);
    assert_round_trip(&packet, &BlockPath::top(0), &text);
    let last_on_first = lines.iter().rfind(|l| l.page == 0).unwrap();
    let pos = packet
        .hit_test(
            0,
            Point::new(last_on_first.rect.x + 50.0, last_on_first.rect.y + 5.0),
        )
        .unwrap();
    let (down, _) = packet
        .move_vertical(&pos, VerticalDirection::Down, None)
        .unwrap();
    assert_eq!(
        packet.caret(&down).unwrap().page,
        1,
        "moving down from the last line crosses the page"
    );
    let (up, _) = packet
        .move_vertical(&down, VerticalDirection::Up, None)
        .unwrap();
    assert_eq!(packet.caret(&up).unwrap().page, 0);
}

#[test]
fn vertical_movement_follows_table_columns() {
    let doc = small(vec![
        para("اوپر"),
        table(vec![
            vec![cell(vec![para("الف")]), cell(vec![para(&words(12))])],
            vec![cell(vec![para("ب")]), cell(vec![para("ج")])],
        ]),
        para("نیچے"),
    ]);
    let (_, packet) = layout(&doc);
    let start = Position::new(0, BlockPath::new(&[1, 0, 0, 0]), 0);
    let (below, _) = packet
        .move_vertical(&start, VerticalDirection::Down, None)
        .unwrap();
    assert_eq!(
        below.path,
        BlockPath::new(&[1, 1, 0, 0]),
        "down stays in the same column"
    );
    let (after, _) = packet
        .move_vertical(&below, VerticalDirection::Down, None)
        .unwrap();
    assert_eq!(after.path, BlockPath::top(2));
    let (above, _) = packet
        .move_vertical(&start, VerticalDirection::Up, None)
        .unwrap();
    assert_eq!(above.path, BlockPath::top(0));
}

#[test]
fn fields_are_atomic_positions() {
    let mut p = Paragraph::new(vec![
        Inline::Text(Text::new("نام ")),
        Inline::Field(Field::new(FieldReference::Unresolved("نام ملزم".into()))),
        Inline::Text(Text::new(" ولد")),
    ]);
    p.props = ElementProps::rtl();
    let doc = small(vec![Block::Paragraph(p.clone())]);
    let (_, packet) = layout(&doc);
    let logical = logical_text(&p.children);
    assert_eq!(logical, "نام \u{FFFC} ولد");
    let field_start = "نام ".len();
    let before = packet
        .caret(&Position::new(0, BlockPath::top(0), field_start))
        .unwrap();
    let after = packet
        .caret(&Position::new(0, BlockPath::top(0), field_start + 3))
        .unwrap();
    assert!(
        (before.rect.x - after.rect.x).abs() > 20.0,
        "field has width"
    );
    let middle = Point::new((before.rect.x + after.rect.x) / 2.0, before.rect.y + 5.0);
    let hit = packet.hit_test(0, middle).unwrap();
    assert!(hit.offset == field_start || hit.offset == field_start + 3);
    let selection = packet.selection(
        &Position::new(0, BlockPath::top(0), field_start),
        &Position::new(0, BlockPath::top(0), field_start + 3),
    );
    assert_eq!(selection.len(), 1);
}

#[test]
fn selection_spans_blocks_and_pages() {
    let doc = small(vec![para(&words(200)), para(&words(10))]);
    let (_, packet) = layout(&doc);
    let rects = packet.selection(
        &Position::new(0, BlockPath::top(0), 10),
        &Position::new(0, BlockPath::top(1), 5),
    );
    let pages: std::collections::BTreeSet<usize> = rects.iter().map(|(p, _)| *p).collect();
    assert!(pages.len() >= 2);
}

#[test]
fn explicit_page_breaks_follow_chromium() {
    let page_count = |blocks: Vec<Block>| layout(&small(blocks)).1.page_count();
    assert_eq!(page_count(vec![para("الف"), page_break(), para("ب")]), 2);
    assert_eq!(
        page_count(vec![page_break(), para("الف")]),
        1,
        "leading break ignored"
    );
    assert_eq!(
        page_count(vec![para("الف"), page_break(), page_break(), para("ب")]),
        3,
        "consecutive breaks leave a blank page"
    );
    assert_eq!(
        page_count(vec![para("الف"), page_break()]),
        2,
        "trailing break adds a page"
    );
}

#[test]
fn widows_and_orphans_hold() {
    for filler in 1..12 {
        let mut blocks: Vec<Block> = (0..filler).map(|_| para("سطر")).collect();
        blocks.push(para(&words(70)));
        let doc = small(blocks);
        let (_, packet) = layout(&doc);
        let target = BlockPath::top(filler);
        let lines: Vec<_> = packet
            .lines(0)
            .into_iter()
            .filter(|l| l.path == target)
            .collect();
        let mut per_page = std::collections::BTreeMap::new();
        for l in &lines {
            *per_page.entry(l.page).or_insert(0usize) += 1;
        }
        if lines.len() >= 4 {
            for (page, count) in &per_page {
                assert!(
                    *count >= 2,
                    "filler {filler}: page {page} has {count} line(s): {per_page:?}"
                );
            }
        }
    }
}

#[test]
fn margins_truncate_after_unforced_breaks_only() {
    let mut blocks: Vec<Block> = (0..40)
        .map(|_| styled_para("سطر", "margin: 0px 0px 30px"))
        .collect();
    blocks.push(page_break());
    blocks.push(styled_para("بعد", "margin: 40px 0px 0px"));
    let doc = small(blocks);
    let (_, packet) = layout(&doc);
    let lines = packet.lines(0);
    let content_top = packet.page(0).unwrap().content.y;
    let second = lines.iter().find(|l| l.page == 1).unwrap();
    assert!(
        (second.rect.y - content_top).abs() < 0.5,
        "unforced break truncates the margin"
    );
    let last = lines.last().unwrap();
    assert!(
        (last.rect.y - content_top - 40.0).abs() < 0.5,
        "forced break keeps the margin"
    );
}

#[test]
fn break_inside_avoid_moves_the_paragraph() {
    let mut blocks: Vec<Block> = (0..9).map(|_| para("سطر")).collect();
    blocks.push(styled_para(&words(40), "break-inside: avoid"));
    let (_, packet) = layout(&small(blocks));
    let pages: std::collections::BTreeSet<usize> = packet
        .lines(0)
        .into_iter()
        .filter(|l| l.path == BlockPath::top(9))
        .map(|l| l.page)
        .collect();
    assert_eq!(pages.len(), 1);
}

#[test]
fn tables_split_by_row_and_repeat_headers() {
    let mut rows = vec![vec![
        header_cell(vec![para("سرخی")]),
        header_cell(vec![para("عنوان")]),
    ]];
    for i in 0..30 {
        rows.push(vec![
            cell(vec![para(&format!("قطار {i}"))]),
            cell(vec![para("قدر")]),
        ]);
    }
    let (_, packet) = layout(&small(vec![table(rows)]));
    assert!(packet.page_count() >= 2);
    let cells = packet.cells(0);
    for page in 1..packet.page_count() {
        assert!(
            cells
                .iter()
                .any(|c| c.page == page && c.path == BlockPath::new(&[0, 0, 0])),
            "header row repeated on page {page}"
        );
    }
    let rows_per_page: Vec<usize> = (0..packet.page_count())
        .map(|p| {
            cells
                .iter()
                .filter(|c| c.page == p && c.path.steps()[2] == 0)
                .count()
        })
        .collect();
    assert!(rows_per_page.iter().all(|n| *n > 1));
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

#[test]
fn packet_documents_start_on_new_pages_with_their_own_size() {
    let a = small(vec![para("الف")]);
    let mut b = Document::new(vec![para("ب")]);
    b.page_layout = PageLayout::new(215.9, 279.4, [12.0; 4]);
    let mut engine = PageEngine::new();
    let packet = engine.layout(&[
        DocSource::new(&a, &TokenFields),
        DocSource::new(&b, &TokenFields),
    ]);
    assert_eq!(packet.page_count(), 2);
    assert!(packet.page(0).unwrap().size.width < packet.page(1).unwrap().size.width);
    assert_eq!(packet.page(1).unwrap().document, 1);
    assert_eq!(packet.document_pages(1), 1..2);
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
        let mut fresh_engine = PageEngine::new();
        let fresh = fresh_engine.layout(&[DocSource::new(&doc, &TokenFields)]);
        assert_eq!(packet.page_count(), fresh.page_count(), "{edit:?}");
        assert_eq!(line_signature(&packet), line_signature(&fresh), "{edit:?}");
    }
}

#[test]
fn small_edits_reuse_the_following_pages() {
    let blocks: Vec<Block> = (0..80).map(|i| para(&words(15 + i % 9))).collect();
    let mut doc = small(blocks);
    let mut engine = PageEngine::new();
    let mut packet = engine.layout(&[DocSource::new(&doc, &TokenFields)]);
    let pages = packet.page_count();
    assert!(pages > 6);
    if let Block::Paragraph(p) = &mut doc.blocks[2]
        && let Inline::Text(t) = &mut p.children[0]
    {
        t.text.insert(0, 'ب');
    }
    engine.update(
        &mut packet,
        0,
        &DocSource::new(&doc, &TokenFields),
        &Edit::Block(BlockPath::top(2)),
    );
    let stats = engine.last_stats();
    assert_eq!(stats.blocks_measured, 1);
    assert_eq!(stats.pages_rebuilt, 1);
    let untouched = packet
        .lines(0)
        .iter()
        .find(|l| l.path == BlockPath::top(2))
        .unwrap()
        .page;
    assert_eq!(untouched + stats.pages_rebuilt + stats.pages_reused, pages);
}

struct Switch(bool);

impl FieldSource for Switch {
    fn display(&self, reference: &FieldReference) -> FieldDisplayText {
        let FieldReference::Unresolved(text) = reference else {
            unreachable!()
        };
        FieldDisplayText {
            text: if self.0 {
                format!("{text} {}", words(8))
            } else {
                text.clone()
            },
            unresolved: !self.0,
        }
    }
}

#[test]
fn field_refresh_touches_only_blocks_with_fields() {
    let mut blocks: Vec<Block> = (0..30).map(|i| para(&words(10 + i))).collect();
    for k in [5usize, 17] {
        blocks[k] = Block::Paragraph(Paragraph::new(vec![
            Inline::Text(Text::new("نام ")),
            Inline::Field(Field::new(FieldReference::Unresolved("نام ملزم".into()))),
        ]));
    }
    let doc = small(blocks);
    let mut engine = PageEngine::new();
    let mut packet = engine.layout(&[DocSource::new(&doc, &Switch(false))]);
    let changed = engine.refresh_fields(&mut packet, &[DocSource::new(&doc, &Switch(true))]);
    assert_eq!(changed, 2);
    assert_eq!(engine.last_stats().blocks_measured, 2);
    let fresh = PageEngine::new().layout(&[DocSource::new(&doc, &Switch(true))]);
    assert_eq!(line_signature(&packet), line_signature(&fresh));
    assert_eq!(
        engine.refresh_fields(&mut packet, &[DocSource::new(&doc, &Switch(true))]),
        0
    );
}
