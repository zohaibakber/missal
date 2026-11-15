use missal_text::{
    Affinity, Align, Attr, AttributedText, Direction, HorizontalDirection, InlineObject,
    LineHeight, ParagraphStyle, TextEngine, TextLayout, TextPosition, TextStyle, word_range_at,
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
fn zero_width_spaces_do_not_create_extra_stops() {
    let mut engine = TextEngine::new();
    let layout = doc(&mut engine, "مدعی مقدمہ", Direction::Rtl, None);
    let stops: Vec<usize> = walk(
        &layout,
        TextPosition::downstream(0),
        HorizontalDirection::Left,
    )
    .iter()
    .map(|(p, _)| p.index)
    .collect();
    let space = "مدعی".len();
    assert!(
        !(stops.contains(&space) && stops.contains(&(space + 1))),
        "both sides of a zero-width space are one visual stop: {stops:?}"
    );
}

#[test]
fn ltr_carets_run_left_to_right() {
    let mut engine = TextEngine::new();
    let layout = engine.layout_uncached(
        &AttributedText::new(LATIN, TextStyle::interface(16.0)),
        &ParagraphStyle::ltr(),
        Some(120.0),
    );
    assert!(!layout.is_rtl());
    assert!(layout.lines().len() > 1, "wraps at 120px");
    assert_eq!(layout.caret(TextPosition::downstream(0)).x, 0.0);
    let right = layout.move_visual(TextPosition::downstream(0), HorizontalDirection::Right);
    assert_eq!(right.index, 1);
    assert_visual_walk(&layout);
    for line in layout.lines() {
        assert!(line.right <= 120.5, "{line:?}");
    }
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
fn ltr_paragraph_with_urdu_inside() {
    let mut engine = TextEngine::new();
    let text = "Filed at تھانہ صدر today";
    let layout = engine.layout_uncached(
        &AttributedText::new(text, TextStyle::interface(16.0)),
        &ParagraphStyle::default(),
        None,
    );
    assert!(!layout.is_rtl(), "first strong character is Latin");
    let urdu = text.find("تھانہ").expect("urdu");
    let start = layout.caret(TextPosition::downstream(urdu)).x;
    let inside = layout
        .caret(TextPosition::downstream(urdu + "تھانہ".len()))
        .x;
    assert!(
        inside < start,
        "inside the Urdu run, later text is further left"
    );
    assert_visual_walk(&layout);
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
fn selection_rects_cover_the_range() {
    let mut engine = TextEngine::new();
    let layout = doc(&mut engine, P3, Direction::Rtl, Some(520.0));
    assert_eq!(layout.lines().len(), 3);
    let all = layout.selection_rects(0..P3.len());
    assert!(all.len() >= 3, "{all:?}");
    let number = P3.find("412/2026").expect("number");
    let urdu_word = P3.find("نمبر").expect("word");
    let rects = layout.selection_rects(urdu_word..number + 3);
    assert!(
        rects.len() >= 2,
        "selecting across a direction change is split visually: {rects:?}"
    );
    let total: f32 = rects.iter().map(|r| r.width).sum();
    assert!(total > 30.0);
    assert!(layout.selection_rects(5..5).is_empty());
}

#[test]
fn trailing_space_before_ltr_run_does_not_overflow() {
    let mut engine = TextEngine::new();
    let layout = doc(&mut engine, P3, Direction::Rtl, Some(520.0));
    for line in layout.lines() {
        assert!(line.left >= -0.5 && line.right <= 520.5, "{line:?}");
    }
    let line = &layout.lines()[1];
    assert!(P3[line.text_range.clone()].ends_with("380 "));
    let seven = P3.find("380").expect("380") + 2;
    let digit = layout.caret(TextPosition::upstream(seven + 1)).x;
    let paren = layout
        .caret(TextPosition::upstream(
            P3.find("(Section").expect("paren") + 1,
        ))
        .x;
    assert!(
        paren - digit < 1.0,
        "the hanging space moves to the line end, not between 380 and the bracket"
    );
}

#[test]
fn vertical_moves_keep_sticky_x() {
    let mut engine = TextEngine::new();
    let layout = doc(&mut engine, P3, Direction::Rtl, Some(520.0));
    let start = layout.hit_test(300.0, layout.lines()[0].baseline);
    let (down, x) = layout.move_vertical(start, 1, None);
    assert_eq!(layout.caret(down).line, 1);
    let (down2, x2) = layout.move_vertical(down, 1, Some(x));
    assert_eq!(layout.caret(down2).line, 2);
    assert_eq!(x, x2);
    let (up, _) = layout.move_vertical(down2, -2, Some(x));
    assert_eq!(layout.caret(up).line, 0);
    assert!((layout.caret(up).x - layout.caret(start).x).abs() < 15.0);
    let (top, _) = layout.move_vertical(start, -1, None);
    assert_eq!(top.index, 0);
    let (bottom, _) = layout.move_vertical(down2, 1, None);
    assert_eq!(bottom.index, P3.len());
}

#[test]
fn line_start_and_end_follow_soft_wraps() {
    let mut engine = TextEngine::new();
    let layout = doc(&mut engine, P3, Direction::Rtl, Some(520.0));
    let second = layout.lines()[1].text_range.clone();
    let inside = TextPosition::downstream(second.start + 20);
    let home = layout.line_start(inside);
    assert_eq!(home, TextPosition::downstream(second.start));
    let end = layout.line_end(inside);
    assert_eq!(end.index, second.end);
    assert_eq!(
        layout.caret(end).line,
        1,
        "end of a wrapped line stays on that line"
    );
    assert_eq!(layout.caret(home).line, 1);
}

#[test]
fn hard_breaks() {
    let mut engine = TextEngine::new();
    let text = "پہلی سطر\nدوسری\n";
    let layout = doc(&mut engine, text, Direction::Rtl, Some(400.0));
    assert_eq!(layout.lines().len(), 3);
    let newline = text.find('\n').expect("newline");
    assert_eq!(
        layout.caret(TextPosition::upstream(newline + 1)).line,
        1,
        "after a newline is the next line"
    );
    assert_eq!(layout.line_end(TextPosition::downstream(0)).index, newline);
    let end = layout.caret(TextPosition::downstream(text.len()));
    assert_eq!(end.line, 2);
    assert!(
        (end.x - 400.0).abs() < 0.5,
        "an empty last RTL line starts at the right"
    );
}

#[test]
fn alignment() {
    let mut engine = TextEngine::new();
    let text = AttributedText::new("یہ سطر", TextStyle::document(24.0));
    let width = 400.0;
    let mut place = |align, dir| {
        let l = engine.layout_uncached(
            &text,
            &ParagraphStyle::default()
                .with_direction(dir)
                .with_align(align),
            Some(width),
        );
        (l.lines()[0].left, l.lines()[0].right)
    };
    assert!((place(Align::Start, Direction::Rtl).1 - width).abs() < 0.5);
    assert!(place(Align::Start, Direction::Ltr).0.abs() < 0.5);
    assert!(place(Align::End, Direction::Rtl).0.abs() < 0.5);
    assert!(place(Align::Left, Direction::Rtl).0.abs() < 0.5);
    let (l, r) = place(Align::Center, Direction::Rtl);
    assert!(((l + r) / 2.0 - width / 2.0).abs() < 0.5);
    let justified = engine.layout_uncached(
        &AttributedText::new(P3, TextStyle::document(24.0)),
        &ParagraphStyle::rtl().with_align(Align::Justify),
        Some(520.0),
    );
    let first = &justified.lines()[0];
    assert!(
        first.left.abs() < 0.5 && (first.right - 520.0).abs() < 0.5,
        "{first:?}"
    );
}

#[test]
fn styles_produce_runs_decorations_and_backgrounds() {
    let mut engine = TextEngine::new();
    let text = "تفتیشی افسر تحریر کنندہ";
    let word = text.find("تحریر").expect("word");
    let attributed = AttributedText::new(text, TextStyle::document(24.0))
        .with(word..word + "تحریر".len(), Attr::Weight(700))
        .with(0..12, Attr::Underline(true))
        .with(
            word..text.len(),
            Attr::Background(missal_text::Rgba::rgb(0xffff00)),
        )
        .with(word..text.len(), Attr::Strikethrough(true));
    let layout = engine.layout_uncached(&attributed, &ParagraphStyle::rtl(), None);
    assert!(
        layout.glyph_runs().iter().any(|r| r.synthesis.embolden),
        "Jameel has no bold face, so bold is synthesised"
    );
    assert!(layout.glyph_runs().iter().any(|r| !r.synthesis.embolden));
    let kinds: Vec<_> = layout.decorations().iter().map(|d| d.kind).collect();
    assert!(kinds.contains(&missal_text::DecorationKind::Underline));
    assert!(kinds.contains(&missal_text::DecorationKind::Strikethrough));
    assert_eq!(layout.backgrounds().len(), 1);
    let underline = layout
        .decorations()
        .iter()
        .find(|d| d.kind == missal_text::DecorationKind::Underline)
        .expect("underline");
    assert!(
        underline.rect.y > layout.lines()[0].baseline,
        "underline sits below the baseline"
    );
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
    let bold = engine.layout_uncached(
        &AttributedText::new("Aslam", TextStyle::interface(14.0).with_weight(700)),
        &ParagraphStyle::default(),
        None,
    );
    assert!(
        bold.glyph_runs()
            .iter()
            .all(|r| !r.synthesis.embolden && missal_text::fonts::face(r.face).weight == 700)
    );
}

#[test]
fn inline_objects_take_space_and_positions() {
    let mut engine = TextEngine::new();
    let text = "نام ملزم";
    let at = text.find(' ').expect("space");
    let attributed =
        AttributedText::new(text, TextStyle::document(24.0)).with_object(InlineObject {
            id: 7,
            index: at,
            width: 60.0,
            height: 30.0,
            baseline: Some(22.0),
        });
    let plain = engine.layout_uncached(
        &AttributedText::new(text, TextStyle::document(24.0)),
        &ParagraphStyle::rtl(),
        None,
    );
    let layout = engine.layout_uncached(&attributed, &ParagraphStyle::rtl(), None);
    assert!((layout.natural_width() - plain.natural_width() - 60.0).abs() < 0.5);
    let object = layout.objects()[0];
    assert_eq!(object.id, 7);
    let before = layout.caret(TextPosition::upstream(at)).x;
    let after = layout.caret(TextPosition::downstream(at)).x;
    assert!(
        (before - object.rect.right()).abs() < 0.5,
        "upstream is the right side of the chip in RTL"
    );
    assert!((after - object.rect.x).abs() < 0.5 || after < object.rect.x + 0.5);
    let left = layout.move_visual(TextPosition::upstream(at), HorizontalDirection::Left);
    assert!(
        layout.caret(left).x <= object.rect.x + 0.5,
        "one left press crosses the chip"
    );
}

#[test]
fn words() {
    let text = "مقدمہ نمبر 412/2026، تھانہ";
    let start = text.find("نمبر").expect("word");
    assert_eq!(word_range_at(text, start + 2), start..start + "نمبر".len());
    let zwnj = "می\u{200C}خواہم بات";
    assert_eq!(
        word_range_at(zwnj, 0),
        0.."می\u{200C}خواہم".len(),
        "ZWNJ stays inside the word"
    );
}

#[test]
fn cache_hits_and_rebreaks() {
    let mut engine = TextEngine::new();
    let text = AttributedText::new(P3, TextStyle::document(24.0));
    let paragraph = ParagraphStyle::rtl();
    let a = engine.layout(&text, &paragraph, Some(520.0));
    let b = engine.layout(&text, &paragraph, Some(520.0));
    assert!(std::sync::Arc::ptr_eq(&a, &b));
    let c = engine.layout(&text, &paragraph, Some(300.0));
    assert!(c.lines().len() > a.lines().len());
    let stats = engine.cache_stats();
    assert_eq!((stats.builds, stats.rebreaks, stats.hits), (1, 1, 1));
    let fresh = engine.layout_uncached(&text, &paragraph, Some(300.0));
    assert_eq!(
        fresh.lines(),
        c.lines(),
        "a re-broken layout matches a fresh one"
    );
    assert_eq!(fresh.glyph_count(), c.glyph_count());
}
