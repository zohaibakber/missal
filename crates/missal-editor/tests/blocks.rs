mod common;

use common::*;
use missal_core::document::{Block, HeadingLevel, Inline, ListType};
use missal_editor::model::blocks::MAX_TABLE_SIDE;
use missal_editor::{BlockCommand, TextKind};
use missal_page::BlockPath;

fn slash(core: &mut missal_editor::EditorCore, steps: &[u32], typed: &str, command: BlockCommand) {
    let text = block_text(core, steps);
    let start = text.rfind(typed).expect("typed trigger");
    core.apply_block_command(&BlockPath::new(steps), start..start + typed.len(), command)
        .expect("command applies");
}

#[test]
fn turning_a_paragraph_into_a_heading_removes_the_trigger_and_undoes_in_one_step() {
    let mut core = core(doc(vec![p("عنوان /h1")]));
    caret(&mut core, &[0], "عنوان /h1".len());
    slash(
        &mut core,
        &[0],
        "/h1",
        BlockCommand::Text(TextKind::Heading(HeadingLevel::H2)),
    );
    match &core.document().blocks[0] {
        Block::Heading(heading) => assert_eq!(heading.level, Some(HeadingLevel::H2)),
        other => panic!("expected a heading, got {other:?}"),
    }
    assert_eq!(texts(&core), ["عنوان "]);
    assert_eq!(head(&core), (vec![0], "عنوان ".len()));
    assert_eq!(core.text_kind(), Some(TextKind::Heading(HeadingLevel::H2)));
    core.undo();
    assert!(matches!(core.document().blocks[0], Block::Paragraph(_)));
    assert_eq!(texts(&core), ["عنوان /h1"]);
}

#[test]
fn quote_and_paragraph_convert_list_items_back_to_text() {
    let mut core = core(doc(vec![p("ایک"), p("دو /")]));
    caret(&mut core, &[1], 0);
    core.toggle_list(ListType::Bullet);
    assert!(core.in_list());
    let path = core.selection().head.path.clone();
    let len = core.block_len(&path).unwrap();
    core.apply_block_command(&path, len - 1..len, BlockCommand::Text(TextKind::Quote))
        .unwrap();
    assert!(!core.in_list());
    assert!(matches!(core.document().blocks[1], Block::Quote(_)));
    assert_eq!(texts(&core), ["ایک", "دو "]);
}

#[test]
fn list_commands_make_a_list_and_leave_an_existing_one_alone() {
    let mut core = core(doc(vec![p("/list")]));
    caret(&mut core, &[0], 5);
    slash(
        &mut core,
        &[0],
        "/list",
        BlockCommand::List(ListType::Number),
    );
    assert_eq!(core.list_state(), Some(ListType::Number));
    let path = core.selection().head.path.clone();
    core.insert_text("/");
    core.apply_block_command(&path, 0..1, BlockCommand::List(ListType::Number))
        .unwrap();
    assert_eq!(core.list_state(), Some(ListType::Number));
    assert_eq!(block_text(&core, path.steps()), "");
}

#[test]
fn a_table_replaces_an_empty_paragraph_and_keeps_a_line_after_it() {
    let mut core = core(doc(vec![p("اوپر"), p("/table")]));
    caret(&mut core, &[1], 6);
    slash(&mut core, &[1], "/table", BlockCommand::table(2, 3));
    let Block::Table(table) = &core.document().blocks[1] else {
        panic!("expected a table");
    };
    assert_eq!(table.rows.len(), 2);
    assert!(table.rows.iter().all(|row| row.cells.len() == 3));
    assert!(matches!(core.document().blocks[2], Block::Paragraph(_)));
    assert_eq!(head(&core), (vec![1, 0, 0, 0], 0));
    core.insert_text("خانہ");
    assert_eq!(block_text(&core, &[1, 0, 0, 0]), "خانہ");
    core.undo();
    core.undo();
    assert_eq!(texts(&core), ["اوپر", "/table"]);
}

#[test]
fn a_divider_after_text_goes_below_it_and_the_caret_moves_past_it() {
    let mut core = core(doc(vec![p("متن /hr"), p("آخر")]));
    caret(&mut core, &[0], "متن /hr".len());
    slash(&mut core, &[0], "/hr", BlockCommand::Divider);
    let blocks = &core.document().blocks;
    assert!(matches!(blocks[0], Block::Paragraph(_)));
    assert!(matches!(blocks[1], Block::HorizontalRule(_)));
    assert_eq!(texts(&core), ["متن ", "آخر"]);
    assert_eq!(head(&core), (vec![2], 0));
}

#[test]
fn a_page_break_at_the_end_adds_a_paragraph_to_keep_typing() {
    let mut core = core(doc(vec![p("/page")]));
    caret(&mut core, &[0], 5);
    slash(&mut core, &[0], "/page", BlockCommand::PageBreak);
    let blocks = &core.document().blocks;
    assert_eq!(blocks.len(), 2);
    assert!(matches!(blocks[0], Block::PageBreak(_)));
    assert_eq!(head(&core), (vec![1], 0));
}

#[test]
fn a_field_command_inserts_the_field_where_the_slash_was() {
    let mut core = core(doc(vec![p("نمبر /مقدمہ")]));
    caret(&mut core, &[0], "نمبر /مقدمہ".len());
    slash(&mut core, &[0], "/مقدمہ", BlockCommand::Field(fir_no()));
    let inlines = core.inlines_at(&BlockPath::new(&[0])).unwrap();
    assert!(matches!(inlines.last(), Some(Inline::Field(_))));
    assert_eq!(core.field_count(), 1);
}

#[test]
fn tables_nest_below_a_filled_cell_paragraph_and_sizes_are_clamped() {
    let mut core = core(doc(vec![grid(1, 1)]));
    caret(&mut core, &[0, 0, 0, 0], 0);
    core.insert_text("/");
    core.apply_block_command(
        &BlockPath::new(&[0, 0, 0, 0]),
        0..1,
        BlockCommand::table(99, 0),
    )
    .unwrap();
    assert_eq!(head(&core), (vec![0, 0, 0, 1, 0, 0, 0], 0));
    assert_eq!(
        BlockCommand::table(99, 0),
        BlockCommand::Table {
            rows: MAX_TABLE_SIDE,
            columns: 1
        }
    );
}
