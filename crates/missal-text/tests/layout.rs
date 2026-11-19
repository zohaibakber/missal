use missal_text::{
    Affinity, AttributedText, Direction, HorizontalDirection, LineHeight, ParagraphStyle,
    TextEngine, TextLayout, TextPosition, TextStyle,
};

const URDU: &str = "مدعی مقدمہ نے تھانہ صدر میں درخواست دی";
const LATIN: &str = "The complainant filed a report";
const MIXED: &str = "مقدمہ نمبر 412/2026 بجرم 379 PPC تھانہ صدر";
const P3: &str = "ایف آئی آر نمبر 412/2026 مورخہ 15-03-2026 کو درج ہوئی، ملزم کا ایمی نمبر 356789012345678 اور سم نمبر 0300-1234567 ہے (Section 380 PPC) جبکہ تحریر کنندہ نے ضمنی میں تاریخ گرفتاری درج کی۔";

fn doc(
    engine: &mut TextEngine,
    text: &str,
    direction: Direction,
    width: Option<f32>,
) -> TextLayout {
    engine.layout_uncached(
        &AttributedText::new(text, TextStyle::document(24.0)),
        &ParagraphStyle::default()
            .with_direction(direction)
            .with_line_height(LineHeight::Relative(2.4)),
        width,
    )
}

fn walk(
    layout: &TextLayout,
    start: TextPosition,
    dir: HorizontalDirection,
) -> Vec<(TextPosition, f32)> {
    let mut out = vec![(start, layout.caret(start).x)];
    let mut pos = start;
    for _ in 0..layout.text().len() * 2 + 4 {
        let next = layout.move_visual(pos, dir);
        if next == pos {
            break;
        }
        pos = next;
        out.push((pos, layout.caret(pos).x));
    }
    out
}

fn assert_visual_walk(layout: &TextLayout) {
    let lines = layout.lines().len();
    let start = if layout.is_rtl() {
        TextPosition::downstream(0)
    } else {
        layout.end_position()
    };
    let leftward = walk(layout, start, HorizontalDirection::Left);
    for pair in leftward.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (la, lb) = (layout.caret(a.0).line, layout.caret(b.0).line);
        if la == lb {
            assert!(
                b.1 < a.1 - 0.4,
                "left must move left on one line: {a:?} -> {b:?} in {:?}",
                layout.text()
            );
        } else {
            let expected = if layout.is_rtl() { la + 1 } else { la - 1 };
            assert_eq!(
                lb, expected,
                "left crosses to the next line in RTL, previous in LTR"
            );
        }
    }
    let end = leftward.last().expect("walk").0;
    let rightward = walk(layout, end, HorizontalDirection::Right);
    let mut back: Vec<usize> = rightward.iter().map(|(p, _)| p.index).collect();
    back.reverse();
    let forth: Vec<usize> = leftward.iter().map(|(p, _)| p.index).collect();
    assert_eq!(back.first(), forth.first());
    assert_eq!(
        back.len(),
        forth.len(),
        "left and right walks visit the same stops ({lines} lines)"
    );
}

#[test]
fn rtl_carets_run_right_to_left() {
    let mut engine = TextEngine::new();
    let layout = doc(&mut engine, URDU, Direction::Rtl, Some(600.0));
    assert!(layout.is_rtl());
    let start = layout.caret(TextPosition::downstream(0));
    let end = layout.caret(layout.end_position());
    assert!(
        (start.x - 600.0).abs() < 0.5,
        "logical start is the right edge: {start:?}"
    );
    assert!(end.x < start.x);
    let mut previous = f32::INFINITY;
    for (index, _) in URDU.char_indices().skip(1) {
        let x = layout
            .caret(TextPosition::downstream(layout.snap_index(index)))
            .x;
        assert!(
            x <= previous + 0.01,
            "caret x never increases along RTL text"
        );
        previous = x;
    }
    assert_visual_walk(&layout);
    let left = layout.move_visual(TextPosition::downstream(0), HorizontalDirection::Left);
    assert_eq!(left.index, 'م'.len_utf8());
    let right = layout.move_visual(TextPosition::downstream(0), HorizontalDirection::Right);
    assert_eq!(
        right.index, 0,
        "nothing to the right of the start of an RTL line"
    );
}

#[test]
fn mixed_bidi_boundaries() {
    let mut engine = TextEngine::new();
    let layout = doc(&mut engine, MIXED, Direction::Rtl, Some(600.0));
    let digits = MIXED.find("412").expect("digits");
    let before = layout.caret(TextPosition::upstream(digits));
    let after = layout.caret(TextPosition::downstream(digits));
    assert!(
        (before.x - after.x).abs() > 20.0,
        "the two affinities at an RTL/LTR boundary are visually apart: {before:?} {after:?}"
    );
    let number_end = digits + "412/2026".len();
    assert!(
        layout.caret(TextPosition::downstream(digits + 1)).x > after.x,
        "digits run left to right"
    );
    assert!(layout.caret(TextPosition::upstream(number_end)).x > after.x);
    assert_visual_walk(&layout);

    let pos = layout.hit_test(after.x + 1.0, 30.0);
    assert_eq!(
        pos.index, digits,
        "clicking just right of the number's left edge"
    );
    assert_eq!(pos.affinity, Affinity::Downstream);
}

#[test]
fn hit_test_round_trips() {
    let mut engine = TextEngine::new();
    for (text, dir) in [
        (URDU, Direction::Rtl),
        (MIXED, Direction::Rtl),
        (LATIN, Direction::Ltr),
        (P3, Direction::Rtl),
    ] {
        let layout = doc(&mut engine, text, dir, Some(400.0));
        for (index, _) in text.char_indices() {
            let index = layout.snap_index(index);
            for affinity in [Affinity::Downstream, Affinity::Upstream] {
                let pos = layout.normalize(TextPosition::new(index, affinity));
                let caret = layout.caret(pos);
                let hit = layout.hit_test(caret.x, (caret.top + caret.bottom) / 2.0);
                let hit_caret = layout.caret(hit);
                assert!(
                    (hit_caret.x - caret.x).abs() < 0.6 && hit_caret.line == caret.line,
                    "{text:?} {pos:?} at {caret:?} hit {hit:?} at {hit_caret:?}"
                );
            }
        }
        let first = layout.lines()[0].clone();
        assert_eq!(
            layout.hit_test(-50.0, first.baseline).index,
            if layout.is_rtl() {
                layout.line_end(TextPosition::downstream(0)).index
            } else {
                0
            }
        );
    }
}

#[test]
fn script_fallback_picks_faces_per_script() {
    let mut engine = TextEngine::new();
    let layout = engine.layout_uncached(
        &AttributedText::new("ملزم Aslam 35", TextStyle::interface(14.0)),
        &ParagraphStyle::default(),
        None,
    );
    let faces: Vec<_> = layout
        .glyph_runs()
        .iter()
        .map(|r| missal_text::fonts::face(r.face).family)
        .collect();
    assert!(faces.contains(&missal_text::URDU_FAMILY), "{faces:?}");
    assert!(faces.contains(&missal_text::UI_FAMILY), "{faces:?}");
    let medium = engine.layout_uncached(
        &AttributedText::new("Aslam", TextStyle::interface(14.0).with_weight(500)),
        &ParagraphStyle::default(),
        None,
    );
    assert!(medium.glyph_runs().iter().all(|r| {
        let face = missal_text::fonts::face(r.face);
        !r.synthesis.embolden && face.family == missal_text::UI_FAMILY && face.weight == 500
    }));
}
