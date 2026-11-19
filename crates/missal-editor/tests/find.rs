mod common;

use common::*;
use missal_core::document::{Inline, TextFormat};
use missal_editor::Query;
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
