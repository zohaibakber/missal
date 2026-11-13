mod common;

use common::*;
use missal_core::document::{Align, Block, Inline, ListType, TextFormat};
use missal_core::FieldReference;
use missal_editor::model::selection::{effective_range, pos};
use missal_editor::{Fragment, Kind, Selection};
use missal_page::{BlockPath, Edit, OBJECT_LEN};

#[test]
fn typing_urdu_into_an_empty_document_and_undo_restores_bytes() {
    let original = missal_core::Document::empty();
    let mut core = core(original.clone());
    for c in "سلام دنیا".chars() {
        core.insert_text(&c.to_string());
    }
    assert_eq!(texts(&core), ["سلام دنیا"]);
    assert_eq!(head(&core), (vec![0], "سلام دنیا".len()));
    assert!(core.is_dirty());
    assert_eq!(core.undo_depth(), 1, "typing coalesces into one step");
    core.undo();
    assert_eq!(core.document().encode(), original.encode());
    assert!(!core.is_dirty(), "undo back to the loaded state is clean");
    core.redo();
    assert_eq!(texts(&core), ["سلام دنیا"]);
}

#[test]
fn typing_in_a_block_reports_a_block_edit() {
    let mut core = core(doc(vec![p("الف"), p("ب")]));
    caret(&mut core, &[1], 0);
    let change = core.insert_text("ج").unwrap();
    assert_eq!(change.edit, Edit::Block(BlockPath::top(1)));
    assert_eq!(change.kind, Kind::Typing);
    let change = core.split().unwrap();
    assert_eq!(change.edit, Edit::Splice { start: 1, removed: 1, inserted: 2 });
}

#[test]
fn coalescing_breaks_on_caret_moves_and_other_kinds() {
    let mut core = core(doc(vec![p("")]));
    core.insert_text("a");
    core.insert_text("b");
    core.set_selection(Selection::caret(at(&[0], 0)));
    core.insert_text("c");
    assert_eq!(texts(&core), ["cab"]);
    assert_eq!(core.undo_depth(), 2);
    caret(&mut core, &[0], 0);
    assert!(core.delete(false, false).is_none());
    assert_eq!(core.undo_depth(), 2, "backspace at the start is a no-op");
    caret(&mut core, &[0], 3);
    core.delete(false, false);
    core.delete(false, false);
    assert_eq!(texts(&core), ["c"]);
    assert_eq!(core.undo_depth(), 3, "consecutive backspaces coalesce");
    core.undo();
    assert_eq!(texts(&core), ["cab"]);
    assert_eq!(head(&core), (vec![0], 3), "selection is restored on undo");
}

#[test]
fn backspace_removes_one_code_point_in_arabic_and_whole_graphemes_elsewhere() {
    let mut core = core(doc(vec![p("بَ e\u{301} 👨\u{200d}👩\u{200d}👧")]));
    let length = core.block_len(&BlockPath::top(0)).unwrap();
    caret(&mut core, &[0], length);
    core.delete(false, false);
    assert_eq!(block_text(&core, &[0]), "بَ e\u{301} ");
    core.delete(false, false);
    core.delete(false, false);
    assert_eq!(block_text(&core, &[0]), "بَ ", "decomposed latin goes as one grapheme");
    core.delete(false, false);
    core.delete(false, false);
    assert_eq!(block_text(&core, &[0]), "ب", "the fatha alone is removed first");
    core.delete(false, false);
    assert_eq!(block_text(&core, &[0]), "");
}

#[test]
fn forward_delete_and_word_deletion() {
    let mut core = core(doc(vec![p("پہلا دوسرا تیسرا")]));
    caret(&mut core, &[0], 0);
    core.delete(true, false);
    assert_eq!(block_text(&core, &[0]), "ہلا دوسرا تیسرا");
    core.delete(true, true);
    assert_eq!(block_text(&core, &[0]), " دوسرا تیسرا");
    let end = core.block_len(&BlockPath::top(0)).unwrap();
    caret(&mut core, &[0], end);
    core.delete(false, true);
    assert_eq!(block_text(&core, &[0]), " دوسرا ");
}

#[test]
fn enter_splits_and_keeps_run_styles_and_paragraph_props() {
    let mut first = missal_core::document::Paragraph::new(vec![
        styled("موٹا", "font-size: 20px", 1),
        styled("سادہ", "color: red", 0),
    ]);
    first.props.align = Some(Align::Center);
    let mut core = core(doc(vec![Block::Paragraph(first)]));
    caret(&mut core, &[0], "موٹا".len() + "سا".len());
    core.split();
    assert_eq!(texts(&core), ["موٹاسا", "دہ"]);
    assert_eq!(head(&core), (vec![1], 0));
    let Block::Paragraph(second) = &core.document().blocks[1] else { panic!() };
    assert_eq!(second.props.align, Some(Align::Center));
    let Inline::Text(run) = &second.children[0] else { panic!() };
    assert_eq!(run.style.as_ref().unwrap().as_str(), "color: red");
    let Block::Paragraph(first) = &core.document().blocks[0] else { panic!() };
    assert_eq!(first.children.len(), 2);
    caret(&mut core, &[1], 0);
    core.delete(false, false);
    assert_eq!(texts(&core), ["موٹاسادہ"]);
    let Block::Paragraph(merged) = &core.document().blocks[0] else { panic!() };
    assert_eq!(merged.children.len(), 2, "runs with different styles stay separate after merging");
    assert_eq!(head(&core), (vec![0], "موٹاسا".len()));
}

#[test]
fn enter_at_edges_and_delete_at_end() {
    let mut core = core(doc(vec![p("الف"), p("ب")]));
    caret(&mut core, &[0], 0);
    core.split();
    assert_eq!(texts(&core), ["", "الف", "ب"]);
    caret(&mut core, &[1], "الف".len());
    core.split();
    assert_eq!(texts(&core), ["", "الف", "", "ب"]);
    caret(&mut core, &[1], "الف".len());
    core.delete(true, false);
    assert_eq!(texts(&core), ["", "الف", "ب"]);
    core.delete(true, false);
    assert_eq!(texts(&core), ["", "الفب"]);
    caret(&mut core, &[0], 0);
    core.delete(false, false);
    assert_eq!(texts(&core), ["", "الفب"], "backspace at document start does nothing");
    let end = core.block_len(&BlockPath::top(1)).unwrap();
    caret(&mut core, &[1], end);
    assert!(core.delete(true, false).is_none(), "delete at document end does nothing");
}

#[test]
fn soft_line_break() {
    let mut core = core(doc(vec![p("اوپرنیچے")]));
    caret(&mut core, &[0], "اوپر".len());
    core.line_break();
    assert_eq!(block_text(&core, &[0]), "اوپر\nنیچے");
    assert_eq!(head(&core), (vec![0], "اوپر".len() + 1));
    core.delete(false, false);
    assert_eq!(block_text(&core, &[0]), "اوپرنیچے");
}

#[test]
fn deleting_and_replacing_across_blocks() {
    let original = doc(vec![p("ایک دو"), p("تین"), p("چار پانچ")]);
    let mut core = core(original.clone());
    select(&mut core, (&[0], "ایک ".len()), (&[2], "چار ".len()));
    core.insert_text("x");
    assert_eq!(texts(&core), ["ایک xپانچ"]);
    assert_eq!(head(&core), (vec![0], "ایک x".len()));
    core.undo();
    assert_eq!(core.document().encode(), original.encode());
    assert_eq!(core.selection().head, at(&[2], "چار ".len()), "the selection comes back");
    core.redo();
    assert_eq!(texts(&core), ["ایک xپانچ"]);
}

#[test]
fn bidi_mixed_text_edits_by_logical_offsets() {
    let mut core = core(doc(vec![p("دفعہ 380 PPC کے تحت")]));
    caret(&mut core, &[0], "دفعہ 380".len());
    core.insert_text("/411");
    assert_eq!(block_text(&core, &[0]), "دفعہ 380/411 PPC کے تحت");
    select(&mut core, (&[0], "دفعہ ".len()), (&[0], "دفعہ 380/411 PPC".len()));
    core.delete(false, false);
    assert_eq!(block_text(&core, &[0]), "دفعہ  کے تحت");
}

#[test]
fn table_cells_edit_in_place_and_report_the_cell() {
    let mut core = core(doc(vec![p("اوپر"), grid(2, 2), p("نیچے")]));
    caret(&mut core, &[1, 0, 1, 0], 0);
    let change = core.insert_text("ز").unwrap();
    assert_eq!(change.edit, Edit::Block(BlockPath::new(&[1, 0, 1, 0])));
    assert_eq!(block_text(&core, &[1, 0, 1, 0]), "زخانہ 01");
    caret(&mut core, &[1, 0, 1, 0], 0);
    assert!(core.delete(false, false).is_none(), "backspace at a cell start stays in the cell");
    let end = core.block_len(&BlockPath::new(&[1, 0, 1, 0])).unwrap();
    caret(&mut core, &[1, 0, 1, 0], end);
    assert!(core.delete(true, false).is_none(), "delete at a cell end stays in the cell");
    core.split();
    assert_eq!(texts(&core)[3], "");
    assert_eq!(head(&core), (vec![1, 0, 1, 1], 0));
}

#[test]
fn tab_moves_between_cells() {
    let mut core = core(doc(vec![grid(2, 2)]));
    caret(&mut core, &[0, 0, 0, 0], 3);
    let next = core.cell_step(false).unwrap();
    assert_eq!(next.head, at(&[0, 0, 1, 0], 0));
    core.set_selection(next);
    let next = core.cell_step(false).unwrap();
    assert_eq!(next.head, at(&[0, 1, 0, 0], 0), "tab wraps into the next row");
    core.set_selection(next);
    assert_eq!(core.cell_step(true).unwrap().head, at(&[0, 0, 1, 0], 0));
    caret(&mut core, &[0, 1, 1, 0], 0);
    assert!(core.cell_step(false).is_none());
}

#[test]
fn selection_across_cells_expands_to_whole_cells_and_clears_them() {
    let mut core = core(doc(vec![grid(2, 3), p("بعد")]));
    select(&mut core, (&[0, 0, 1, 0], 2), (&[0, 1, 0, 0], 3));
    let (s, e) = effective_range(core.document(), core.selection());
    assert_eq!(s, at(&[0, 0, 1, 0], 0));
    assert_eq!(e.path, BlockPath::new(&[0, 1, 0, 0]));
    assert_eq!(e.offset, "خانہ 10".len());
    core.insert_text("ق");
    let all = texts(&core);
    assert_eq!(all, ["خانہ 00", "ق", "", "", "خانہ 11", "خانہ 12", "بعد"]);
    let Block::Table(table) = &core.document().blocks[0] else { panic!() };
    assert_eq!(table.rows.len(), 2, "the table keeps its shape");
}

#[test]
fn selection_from_a_paragraph_into_a_table_trims_and_clears() {
    let mut core = core(doc(vec![p("پہلے"), grid(1, 2), p("بعد")]));
    select(&mut core, (&[0], 2), (&[1, 0, 0, 0], 2));
    core.delete(false, false);
    assert_eq!(texts(&core), ["پ", "", "خانہ 01", "بعد"]);
    core.undo();
    select(&mut core, (&[0], 2), (&[2], 2));
    core.delete(false, false);
    assert_eq!(texts(&core), ["پعد"], "a table strictly inside the range is removed");
    assert_eq!(core.document().blocks.len(), 1);
}

#[test]
fn alignment_applies_to_every_selected_block() {
    let mut core = core(doc(vec![p("ایک"), p("دو"), p("تین")]));
    assert_eq!(core.align(), Align::Right, "rtl paragraphs default to right");
    select(&mut core, (&[0], 1), (&[1], 1));
    let change = core.set_align(Align::Center).unwrap();
    assert_eq!(change.kind, Kind::Format);
    assert_eq!(core.align(), Align::Center);
    caret(&mut core, &[2], 0);
    assert_eq!(core.align(), Align::Right);
    core.undo();
    caret(&mut core, &[0], 0);
    assert_eq!(core.align(), Align::Right);
}

#[test]
fn bullet_and_numbered_lists_toggle_indent_and_exit() {
    let original = doc(vec![p("ایک"), p("دو"), p("تین")]);
    let mut core = core(original.clone());
    select(&mut core, (&[0], 0), (&[1], 1));
    core.toggle_list(ListType::Bullet);
    assert!(matches!(core.document().blocks[0], Block::List(_)));
    assert_eq!(texts(&core), ["ایک", "دو", "تین"]);
    assert_eq!(core.list_state(), Some(ListType::Bullet));
    assert_eq!(core.selection().head.path, BlockPath::new(&[0, 1]));
    core.toggle_list(ListType::Number);
    assert_eq!(core.list_state(), Some(ListType::Number));
    caret(&mut core, &[0, 1], 0);
    core.indent(false);
    assert_eq!(core.selection().head.path, BlockPath::new(&[0, 1, 0, 0]));
    assert_eq!(texts(&core), ["ایک", "دو", "تین"]);
    core.indent(true);
    assert_eq!(core.selection().head.path, BlockPath::new(&[0, 1]));
    let end = core.block_len(&BlockPath::new(&[0, 1])).unwrap();
    caret(&mut core, &[0, 1], end);
    core.split();
    assert_eq!(texts(&core), ["ایک", "دو", "", "تین"]);
    core.split();
    assert_eq!(texts(&core), ["ایک", "دو", "", "تین"], "enter on an empty item leaves the list");
    assert_eq!(head(&core), (vec![1], 0));
    assert!(matches!(core.document().blocks[1], Block::Paragraph(_)));
    caret(&mut core, &[0, 1], 0);
    core.delete(false, false);
    assert!(matches!(core.document().blocks[1], Block::Paragraph(_)), "backspace at an item start removes the bullet");
    while core.undo().is_some() {}
    assert_eq!(core.document().encode(), original.encode());
}

#[test]
fn list_toggle_off_restores_paragraphs() {
    let mut core = core(doc(vec![p("ایک"), p("دو")]));
    select(&mut core, (&[0], 0), (&[1], 0));
    core.toggle_list(ListType::Bullet);
    select(&mut core, (&[0, 0], 0), (&[0, 1], 0));
    core.toggle_list(ListType::Bullet);
    assert!(core.document().blocks.iter().all(|b| matches!(b, Block::Paragraph(_))));
    assert_eq!(texts(&core), ["ایک", "دو"]);
}

#[test]
fn bold_toggles_on_mixed_selection_and_off_when_uniform() {
    let mut core = core(doc(vec![para(vec![styled("موٹا", "font-weight: bold", 0), text(" سادہ")])]));
    let length = core.block_len(&BlockPath::top(0)).unwrap();
    select(&mut core, (&[0], 0), (&[0], length));
    assert!(!core.flag_state(TextFormat::BOLD));
    core.toggle_flag(TextFormat::BOLD);
    assert!(core.flag_state(TextFormat::BOLD), "mixed selection turns on");
    core.toggle_flag(TextFormat::BOLD);
    assert!(!core.flag_state(TextFormat::BOLD), "uniform bold turns off, including css bold");
    let inlines = core.inlines_at(&BlockPath::top(0)).unwrap();
    for inline in &inlines {
        if let Inline::Text(t) = inline {
            assert!(!t.format().contains(TextFormat::BOLD));
            assert!(t.style.as_ref().is_none_or(|s| s.get("font-weight").is_none()));
        }
    }
    assert_eq!(core.undo_depth(), 2, "each format toggle is its own step");
}

#[test]
fn pending_format_applies_to_typing_at_a_collapsed_caret() {
    let mut core = core(doc(vec![p("سادہ")]));
    caret(&mut core, &[0], "سادہ".len());
    core.toggle_flag(TextFormat::ITALIC);
    assert!(core.flag_state(TextFormat::ITALIC));
    core.insert_text("ترچھا");
    let inlines = core.inlines_at(&BlockPath::top(0)).unwrap();
    assert_eq!(inlines.len(), 2);
    let Inline::Text(last) = &inlines[1] else { panic!() };
    assert!(last.format().contains(TextFormat::ITALIC));
    core.insert_text("ہ");
    assert_eq!(core.inlines_at(&BlockPath::top(0)).unwrap().len(), 2);
    caret(&mut core, &[0], 0);
    assert!(!core.flag_state(TextFormat::ITALIC), "moving the caret drops the pending format");
}

#[test]
fn fields_are_atomic_for_deletion() {
    let mut core = core(doc(vec![para(vec![text("نمبر "), field(fir_no()), text(" درج")])]));
    let after_field = "نمبر ".len() + OBJECT_LEN;
    caret(&mut core, &[0], after_field);
    core.delete(false, false);
    assert_eq!(core.field_count(), 0);
    assert_eq!(block_text(&core, &[0]), "نمبر  درج");
    core.undo();
    caret(&mut core, &[0], "نمبر ".len());
    core.delete(true, false);
    assert_eq!(core.field_count(), 0);
}

#[test]
fn typing_markers_converts_when_the_close_marker_is_typed() {
    let mut core = core(doc(vec![p("")]));
    for c in "مقدمہ «جرم» درج".chars() {
        core.insert_text(&c.to_string());
    }
    assert_eq!(core.field_count(), 1);
    let inlines = core.inlines_at(&BlockPath::top(0)).unwrap();
    assert!(inlines.iter().any(|i| matches!(i, Inline::Field(f) if f.reference == offence())));
    assert_eq!(block_text(&core, &[0]), format!("مقدمہ {} درج", missal_page::OBJECT));
    assert_eq!(head(&core).1, core.block_len(&BlockPath::top(0)).unwrap());
    core.undo();
    assert_eq!(block_text(&core, &[0]), "", "conversion is part of the typing step");
}

#[test]
fn tokens_wait_while_the_caret_is_inside_and_unknown_names_stay_text() {
    let mut core = core(doc(vec![p("«جم»")]));
    caret(&mut core, &[0], "«ج".len());
    core.insert_text("ر");
    assert_eq!(core.field_count(), 0, "caret still inside the token");
    caret(&mut core, &[0], 0);
    assert_eq!(core.field_count(), 1, "leaving the token converts it");
    assert_eq!(head(&core), (vec![0], 0));
    let mut other = common::core(doc(vec![p("")]));
    for c in "«نامعلوم»".chars() {
        other.insert_text(&c.to_string());
    }
    assert_eq!(other.field_count(), 0);
}

#[test]
fn open_marker_with_selection_wraps_it_into_a_field() {
    let mut core = core(doc(vec![p("نام بینک یہاں")]));
    select(&mut core, (&[0], "نام ".len()), (&[0], "نام بینک".len()));
    core.insert_text("«");
    let inlines = core.inlines_at(&BlockPath::top(0)).unwrap();
    assert!(inlines.iter().any(|i| matches!(i, Inline::Field(f) if f.reference == FieldReference::Catalog(missal_core::PlaceholderId::new(2).unwrap()))));
    select(&mut core, (&[0], 0), (&[0], "نام".len()));
    core.insert_text("«");
    let inlines = core.inlines_at(&BlockPath::top(0)).unwrap();
    assert!(inlines.iter().any(|i| matches!(i, Inline::Field(f) if f.reference == FieldReference::Unresolved("نام".into()))));
}

#[test]
fn reveal_turns_a_field_into_editable_text_with_the_name_selected() {
    let mut core = core(doc(vec![para(vec![text("الف "), field(offence()), text(" ب")])]));
    core.reveal_field(&BlockPath::top(0), "الف ".len());
    assert_eq!(block_text(&core, &[0]), "الف «جرم» ب");
    let selection = core.selection().clone();
    assert_eq!(selection.anchor.offset, "الف «".len());
    assert_eq!(selection.head.offset, "الف «جرم".len());
    assert_eq!(core.field_count(), 0);
    core.insert_text("تفتیشی");
    assert_eq!(core.field_count(), 0);
    let end = core.block_len(&BlockPath::top(0)).unwrap();
    caret(&mut core, &[0], end);
    assert_eq!(core.field_count(), 1, "moving out converts the edited name back");
}

#[test]
fn composition_defers_conversion_until_commit() {
    let mut core = core(doc(vec![p("")]));
    core.insert_text("«جرم");
    core.compose("»", None);
    assert!(core.marked().is_some());
    assert_eq!(core.field_count(), 0, "no conversion while composing");
    core.commit("»");
    assert!(core.marked().is_none());
    assert_eq!(core.field_count(), 1);
}

#[test]
fn composition_replaces_marked_text_and_is_one_undo_step() {
    let mut core = core(doc(vec![p("نام ")]));
    caret(&mut core, &[0], "نام ".len());
    core.compose("ک", None);
    assert_eq!(core.marked().unwrap().range, "نام ".len().."نام ک".len());
    core.compose("کی", Some(2..2));
    assert_eq!(block_text(&core, &[0]), "نام کی");
    assert_eq!(head(&core).1, "نام کی".len());
    core.commit("کیا");
    assert_eq!(block_text(&core, &[0]), "نام کیا");
    core.undo();
    assert_eq!(block_text(&core, &[0]), "نام ");
    core.compose("´", None);
    core.compose("", None);
    assert!(core.marked().is_none());
    assert_eq!(block_text(&core, &[0]), "نام ");
}

#[test]
fn copy_and_paste_round_trip_fields_and_formatting() {
    let source = doc(vec![
        para(vec![styled("موٹا ", "", 1), field(fir_no()), text(" سادہ")]),
        p("دوسرا"),
    ]);
    let mut core = core(source);
    select(&mut core, (&[0], 0), (&[1], "دو".len()));
    let fragment = core.copy().unwrap();
    let decoded = Fragment::decode(&fragment.encode()).unwrap();
    assert_eq!(decoded, fragment);
    let plain = missal_editor::model::clip::plain_text(&fragment, &|_| missal_core::FieldDisplayText {
        text: "12/26".into(),
        unresolved: false,
    });
    assert_eq!(plain, "موٹا 12/26 سادہ\nدو");
    let end = core.block_len(&BlockPath::top(1)).unwrap();
    caret(&mut core, &[1], end);
    core.paste_fragment(decoded);
    assert_eq!(core.field_count(), 2);
    assert_eq!(texts(&core)[1], format!("دوسراموٹا {} سادہ", missal_page::OBJECT));
    assert_eq!(texts(&core)[2], "دو");
    let inlines = core.inlines_at(&BlockPath::top(1)).unwrap();
    assert!(inlines.iter().any(|i| matches!(i, Inline::Text(t) if t.format().contains(TextFormat::BOLD))));
}

#[test]
fn inline_fragment_pastes_inside_the_paragraph() {
    let mut core = core(doc(vec![para(vec![text("الف"), field(offence()), text("ب")])]));
    select(&mut core, (&[0], "الف".len()), (&[0], "الف".len() + OBJECT_LEN));
    let fragment = core.copy().unwrap();
    assert!(fragment.inline);
    caret(&mut core, &[0], 0);
    core.paste_fragment(fragment);
    assert_eq!(core.field_count(), 2);
    assert_eq!(texts(&core).len(), 1);
    assert_eq!(head(&core), (vec![0], OBJECT_LEN));
}

#[test]
fn plain_text_paste_splits_lines_and_converts_tokens() {
    let mut core = core(doc(vec![p("شروع")]));
    caret(&mut core, &[0], "شروع".len());
    core.paste_text(" ایک\r\nدو «جرم»\nتین");
    assert_eq!(texts(&core)[0], "شروع ایک");
    assert_eq!(texts(&core)[2], "تین");
    assert_eq!(core.field_count(), 1);
    assert_eq!(head(&core), (vec![2], "تین".len()));
    core.undo();
    assert_eq!(texts(&core), ["شروع"]);
}

#[test]
fn html_paste_degrades_to_paragraphs() {
    let mut core = core(doc(vec![p("")]));
    core.paste_text("<html><head><style>p{}</style></head><body><p>ایک &amp; دو</p><p>تین<br>چار</p></body></html>");
    assert_eq!(texts(&core), ["ایک & دو", "تین", "چار"]);
}

#[test]
fn undo_depth_is_bounded_at_one_hundred() {
    let mut core = core(doc(vec![p("")]));
    for _ in 0..130 {
        core.split();
    }
    assert_eq!(core.undo_depth(), 100);
}

#[test]
fn revision_rule_for_saves() {
    let mut core = core(doc(vec![p("")]));
    core.insert_text("الف");
    let (_, revision) = core.snapshot();
    core.insert_text("ب");
    core.mark_saved(revision);
    assert!(core.is_dirty(), "edits after the snapshot keep it dirty");
    let (_, revision) = core.snapshot();
    core.mark_saved(revision);
    assert!(!core.is_dirty());
    core.insert_text("ج");
    core.discard();
    assert_eq!(texts(&core), ["الفب"], "discard returns to the saved state");
    assert!(!core.is_dirty());
}

#[test]
fn select_all_selects_the_cell_then_the_document() {
    let mut core = core(doc(vec![p("اوپر"), grid(1, 2)]));
    caret(&mut core, &[1, 0, 1, 0], 2);
    core.select_all();
    let (s, e) = core.selection().ordered();
    assert_eq!(s, at(&[1, 0, 1, 0], 0));
    assert_eq!(e.path, BlockPath::new(&[1, 0, 1, 0]));
    core.select_all();
    let (s, e) = core.selection().ordered();
    assert_eq!(s.path, BlockPath::top(0));
    assert_eq!(e.path, BlockPath::new(&[1, 0, 1, 0]));
}

#[test]
fn empty_documents_and_documents_without_text_blocks() {
    let mut core = core(doc(vec![Block::PageBreak(Default::default())]));
    assert_eq!(core.document().blocks.len(), 2, "a paragraph is provided for the caret");
    core.insert_text("ا");
    caret(&mut core, &[1], 0);
    core.delete(false, false);
    assert_eq!(core.document().blocks.len(), 1, "backspace after a page break removes it");
    assert_eq!(head(&core), (vec![0], 0));
}

#[test]
fn selection_validity_is_restored_after_external_edits() {
    let mut core = core(doc(vec![p("ایک"), p("دو")]));
    core.set_selection(Selection::caret(pos(BlockPath::top(7), 99)));
    assert!(core.is_valid(&core.selection().head));
}

fn edits_match_fresh_layout(document: &missal_core::Document, engine: &mut missal_page::PageEngine, packet: &missal_page::Packet) {
    let fields = missal_page::TokenFields;
    let fresh = missal_page::PageEngine::new().layout(&[missal_page::DocSource::new(document, &fields)]);
    assert_eq!(packet.page_count(), fresh.page_count());
    let a: Vec<_> = packet.lines(0).into_iter().map(|l| (l.path, l.page, l.text, (l.rect.y * 10.0).round() as i64)).collect();
    let b: Vec<_> = fresh.lines(0).into_iter().map(|l| (l.path, l.page, l.text, (l.rect.y * 10.0).round() as i64)).collect();
    assert_eq!(a, b);
    let _ = engine;
}

#[test]
fn incremental_relayout_matches_full_layout_across_page_boundaries() {
    let words = "مدعی مقدمہ نے تھانہ صدر میں درخواست دی کہ مورخہ بارہ مارچ کی رات";
    let blocks: Vec<Block> = (0..30).map(|i| p(&format!("{i} {words}"))).collect();
    let document = small_page(doc(blocks));
    let mut core = core(document);
    let fields = missal_page::TokenFields;
    let mut engine = missal_page::PageEngine::new();
    let mut packet = engine.layout(&[missal_page::DocSource::new(core.document(), &fields)]);
    assert!(packet.page_count() > 3);
    let lines = packet.lines(0);
    let last_on_first = lines.iter().rfind(|l| l.page == 0).unwrap().clone();
    caret(&mut core, last_on_first.path.steps(), last_on_first.text.end);
    let mut apply = |core: &mut missal_editor::EditorCore, change: Option<missal_editor::Change>| {
        if let Some(change) = change {
            engine.update(&mut packet, 0, &missal_page::DocSource::new(core.document(), &fields), &change.edit);
        }
    };
    for _ in 0..40 {
        let change = core.insert_text("ک");
        apply(&mut core, change);
    }
    let change = core.split();
    apply(&mut core, change);
    let change = core.delete(false, false);
    apply(&mut core, change);
    select(&mut core, (&[2], 3), (&[9], 4));
    let change = core.delete(false, false);
    apply(&mut core, change);
    let change = core.undo();
    apply(&mut core, change);
    let mut fresh_engine = missal_page::PageEngine::new();
    edits_match_fresh_layout(core.document(), &mut fresh_engine, &packet);
    let caret_page = packet.caret(&core.selection().head).unwrap().page;
    assert!(caret_page < packet.page_count());
}
