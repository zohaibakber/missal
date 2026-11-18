mod common;

use common::*;
use missal_core::document::{Inline, ListType, TextFormat};
use missal_editor::model::find::{after, at_or_after, before, exact, find_all};
use missal_editor::{Match, Query};
use missal_page::BlockPath;

fn spans(
    core: &missal_editor::EditorCore,
    query: &str,
    match_case: bool,
) -> Vec<(Vec<u32>, String)> {
    core.find(&Query::new(query, match_case))
        .into_iter()
        .map(|m| {
            let text = core.block_text(&m.path).unwrap();
            (m.path.steps().to_vec(), text[m.range].to_owned())
        })
        .collect()
}

#[test]
fn case_folding_is_optional() {
    let core = core(doc(vec![p("FIR fir Fir"), p("no match here")]));
    assert_eq!(spans(&core, "fir", false).len(), 3);
    assert_eq!(spans(&core, "fir", true), [(vec![0], "fir".to_owned())]);
    assert_eq!(spans(&core, "FIR", true), [(vec![0], "FIR".to_owned())]);
    assert!(spans(&core, "", false).is_empty());
}

#[test]
fn urdu_spellings_and_marks_fold_unless_matching_exactly() {
    let core = core(doc(vec![
        p("كتاب اور کتاب"),
        p("مُقدّمہ نمبر ۱۲۳"),
        p("علي اور علی"),
    ]));
    assert_eq!(
        spans(&core, "کتاب", false),
        [(vec![0], "كتاب".to_owned()), (vec![0], "کتاب".to_owned())]
    );
    assert_eq!(spans(&core, "کتاب", true), [(vec![0], "کتاب".to_owned())]);
    assert_eq!(
        spans(&core, "مقدمہ", false),
        [(vec![1], "مُقدّمہ".to_owned())],
        "diacritics are ignored and kept inside the match"
    );
    assert_eq!(spans(&core, "123", false), [(vec![1], "۱۲۳".to_owned())]);
    assert!(spans(&core, "123", true).is_empty());
    assert_eq!(spans(&core, "علی", false).len(), 2);
}

#[test]
fn matches_never_cross_fields_or_lines() {
    let core = core(doc(vec![
        para(vec![text("تھانہ "), field(fir_no()), text(" نمبر")]),
        para(vec![
            text("پہلی"),
            Inline::LineBreak(Default::default()),
            text("سطر"),
        ]),
    ]));
    assert!(spans(&core, "تھانہ  نمبر", false).is_empty());
    assert_eq!(spans(&core, "نمبر", false), [(vec![0], "نمبر".to_owned())]);
    assert!(spans(&core, "پہلی سطر", false).is_empty());
    assert!(spans(&core, "\u{FFFC}", false).is_empty());
}

#[test]
fn matches_cover_tables_and_lists_in_document_order() {
    let mut core = core(doc(vec![p("خانہ اول"), grid(2, 2), p("آخر"), p("خانہ")]));
    caret(&mut core, &[3], 0);
    core.toggle_list(ListType::Bullet);
    let found = spans(&core, "خانہ", false);
    let paths: Vec<Vec<u32>> = found.into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        paths,
        [
            vec![0],
            vec![1, 0, 0, 0],
            vec![1, 0, 1, 0],
            vec![1, 1, 0, 0],
            vec![1, 1, 1, 0],
            vec![3, 0],
        ]
    );
}

#[test]
fn navigation_wraps_in_both_directions() {
    let document = doc(vec![p("ab ab"), p("ab")]);
    let matches = find_all(&document, &Query::new("ab", false));
    assert_eq!(matches.len(), 3);
    let at = |steps: &[u32], offset| at(steps, offset);
    assert_eq!(at_or_after(&matches, &at(&[0], 0)), Some(0));
    assert_eq!(at_or_after(&matches, &at(&[0], 1)), Some(1));
    assert_eq!(after(&matches, &at(&[0], 3)), Some(2));
    assert_eq!(after(&matches, &at(&[1], 0)), Some(0));
    assert_eq!(before(&matches, &at(&[0], 0)), Some(2));
    assert_eq!(before(&matches, &at(&[1], 0)), Some(1));
    assert_eq!(exact(&matches, &at(&[0], 3), &at(&[0], 5)), Some(1));
    assert_eq!(exact(&matches, &at(&[0], 3), &at(&[0], 4)), None);
    assert_eq!(at_or_after(&[], &at(&[0], 0)), None);
}

#[test]
fn replacing_one_match_keeps_its_formatting_and_undoes_in_one_step() {
    let mut core = core(doc(vec![para(vec![
        text("ملزم "),
        styled("زید", "", TextFormat::BOLD.0),
        text(" حاضر"),
    ])]));
    let found = core.find(&Query::new("زید", true));
    assert_eq!(found.len(), 1);
    core.replace_matches(&found, "بکر");
    assert_eq!(block_text(&core, &[0]), "ملزم بکر حاضر");
    let inlines = core.inlines_at(&BlockPath::top(0)).unwrap();
    assert!(inlines.iter().any(|i| matches!(
        i,
        Inline::Text(t) if t.text == "بکر" && t.format().contains(TextFormat::BOLD)
    )));
    assert_eq!(head(&core), (vec![0], "ملزم بکر".len()));
    core.undo();
    assert_eq!(block_text(&core, &[0]), "ملزم زید حاضر");
}

#[test]
fn replace_all_spans_blocks_and_is_one_undo_step() {
    let mut core = core(doc(vec![p("گواہ اول، گواہ دوم"), grid(1, 1), p("گواہ")]));
    let found = core.find(&Query::new("گواہ", false));
    assert_eq!(found.len(), 3);
    core.replace_matches(&found, "گواہان گواہ");
    assert_eq!(
        texts(&core),
        ["گواہان گواہ اول، گواہان گواہ دوم", "خانہ 00", "گواہان گواہ"]
    );
    assert_eq!(core.find(&Query::new("گواہ", false)).len(), 6);
    core.undo();
    assert_eq!(texts(&core), ["گواہ اول، گواہ دوم", "خانہ 00", "گواہ"]);
}

#[test]
fn replacing_with_nothing_deletes_and_stale_matches_are_skipped() {
    let mut core = core(doc(vec![p("abc abc")]));
    let found = core.find(&Query::new("abc", false));
    core.replace_matches(&found, "");
    assert_eq!(block_text(&core, &[0]), " ");
    let stale = vec![Match {
        path: BlockPath::top(0),
        range: 4..7,
    }];
    assert!(core.replace_matches(&stale, "x").is_none());
    assert_eq!(block_text(&core, &[0]), " ");
}

#[test]
fn whole_bundle_search_finds_every_occurrence() {
    for (name, document) in bundle() {
        let core = core(document);
        for word in ["مقدمہ", "تھانہ", "the"] {
            let found = core.find(&Query::new(word, false));
            for m in &found {
                let text = core.block_text(&m.path).unwrap();
                assert!(text.is_char_boundary(m.range.start), "{name}");
                assert!(text.is_char_boundary(m.range.end), "{name}");
            }
            let mut sorted = found.clone();
            sorted.sort_by(|a, b| a.path.cmp(&b.path).then(a.range.start.cmp(&b.range.start)));
            assert_eq!(sorted, found, "{name}: matches are in document order");
        }
    }
}
