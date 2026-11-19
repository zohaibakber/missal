mod common;

use common::*;
use missal_core::document::{Block, Inline, TextFormat};
use missal_editor::Fragment;
use missal_page::{BlockPath, OBJECT_LEN};

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
fn backspace_removes_one_code_point_in_arabic_and_whole_graphemes_elsewhere() {
    let mut core = core(doc(vec![p("بَ e\u{301} 👨\u{200d}👩\u{200d}👧")]));
    let length = core.block_len(&BlockPath::top(0)).unwrap();
    caret(&mut core, &[0], length);
    core.delete(false, false);
    assert_eq!(block_text(&core, &[0]), "بَ e\u{301} ");
    core.delete(false, false);
    core.delete(false, false);
    assert_eq!(
        block_text(&core, &[0]),
        "بَ ",
        "decomposed latin goes as one grapheme"
    );
    core.delete(false, false);
    core.delete(false, false);
    assert_eq!(
        block_text(&core, &[0]),
        "ب",
        "the fatha alone is removed first"
    );
    core.delete(false, false);
    assert_eq!(block_text(&core, &[0]), "");
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
    assert_eq!(
        core.selection().head,
        at(&[2], "چار ".len()),
        "the selection comes back"
    );
    core.redo();
    assert_eq!(texts(&core), ["ایک xپانچ"]);
}

#[test]
fn bidi_mixed_text_edits_by_logical_offsets() {
    let mut core = core(doc(vec![p("دفعہ 380 PPC کے تحت")]));
    caret(&mut core, &[0], "دفعہ 380".len());
    core.insert_text("/411");
    assert_eq!(block_text(&core, &[0]), "دفعہ 380/411 PPC کے تحت");
    select(
        &mut core,
        (&[0], "دفعہ ".len()),
        (&[0], "دفعہ 380/411 PPC".len()),
    );
    core.delete(false, false);
    assert_eq!(block_text(&core, &[0]), "دفعہ  کے تحت");
}

#[test]
fn fields_are_atomic_for_deletion() {
    let mut core = core(doc(vec![para(vec![
        text("نمبر "),
        field(fir_no()),
        text(" درج"),
    ])]));
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
    let plain =
        missal_editor::model::clip::plain_text(&fragment, &|_| missal_core::FieldDisplayText {
            text: "12/26".into(),
            unresolved: false,
        });
    assert_eq!(plain, "موٹا 12/26 سادہ\nدو");
    let end = core.block_len(&BlockPath::top(1)).unwrap();
    caret(&mut core, &[1], end);
    core.paste_fragment(decoded);
    assert_eq!(core.field_count(), 2);
    assert_eq!(
        texts(&core)[1],
        format!("دوسراموٹا {} سادہ", missal_page::OBJECT)
    );
    assert_eq!(texts(&core)[2], "دو");
    let inlines = core.inlines_at(&BlockPath::top(1)).unwrap();
    assert!(
        inlines
            .iter()
            .any(|i| matches!(i, Inline::Text(t) if t.format().contains(TextFormat::BOLD)))
    );
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

fn edits_match_fresh_layout(
    document: &missal_core::Document,
    engine: &mut missal_page::PageEngine,
    packet: &missal_page::Packet,
) {
    let fields = missal_page::TokenFields;
    let fresh = missal_page::PageEngine::with_cache(missal_page::LayoutCache::default())
        .layout(&[missal_page::DocSource::new(document, &fields)]);
    assert_eq!(packet.page_count(), fresh.page_count());
    let a: Vec<_> = packet
        .lines(0)
        .into_iter()
        .map(|l| (l.path, l.page, l.text, (l.rect.y * 10.0).round() as i64))
        .collect();
    let b: Vec<_> = fresh
        .lines(0)
        .into_iter()
        .map(|l| (l.path, l.page, l.text, (l.rect.y * 10.0).round() as i64))
        .collect();
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
    caret(
        &mut core,
        last_on_first.path.steps(),
        last_on_first.text.end,
    );
    let mut apply = |core: &mut missal_editor::EditorCore,
                     change: Option<missal_editor::Change>| {
        if let Some(change) = change {
            engine.update(
                &mut packet,
                0,
                &missal_page::DocSource::new(core.document(), &fields),
                &change.edit,
            );
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

#[test]
fn saving_an_older_snapshot_while_typing_continues() {
    let mut core = core(doc(vec![p("")]));
    core.insert_text("ا");
    core.insert_text("ب");
    let (saved, revision) = core.snapshot();
    core.insert_text("ج");
    core.insert_text("د");
    core.mark_saved(revision);
    assert!(core.is_dirty());
    core.discard();
    assert_eq!(core.document().encode(), saved.encode());
    assert!(!core.is_dirty());
    let mut dirty = common::core(doc(vec![p("")]));
    dirty.load_dirty(doc(vec![p("درآمد")]));
    assert!(dirty.is_dirty(), "an imported document starts unsaved");
    let (_, revision) = dirty.snapshot();
    dirty.mark_saved(revision);
    assert!(!dirty.is_dirty());
}

#[test]
fn a_restored_draft_is_unsaved_and_one_undo_returns_to_the_saved_document() {
    let saved = doc(vec![p("محفوظ")]);
    let mut core = core(doc(vec![p("")]));
    core.restore(saved.clone(), doc(vec![p("مسودہ"), p("دوسرا")]));
    assert_eq!(texts(&core), ["مسودہ", "دوسرا"]);
    assert!(core.is_dirty());
    core.insert_text("ا");
    assert_eq!(
        core.undo_depth(),
        2,
        "typing after a restore is its own step"
    );
    core.undo();
    core.undo();
    assert_eq!(core.document().encode(), saved.encode());
    assert!(!core.is_dirty());
    core.redo();
    assert_eq!(texts(&core), ["مسودہ", "دوسرا"]);
    core.discard();
    assert_eq!(core.document().encode(), saved.encode());
    assert!(!core.is_dirty());
}
