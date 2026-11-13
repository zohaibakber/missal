use missal_text::{Attr, AttributedText, ParagraphStyle, TextEngine, TextStyle};

#[test]
fn word_spacing_probe() {
    let mut engine = TextEngine::new();
    let text = "مدعی مقدمہ نے تھانہ";
    let base = AttributedText::new(text, TextStyle::document(21.333));
    let mut spaced = base.clone();
    for (i, c) in text.char_indices() {
        if c == ' ' {
            spaced.push(i..i + 1, Attr::Size(41.33));
        }
    }
    let a = engine.layout_uncached(&base, &ParagraphStyle::rtl(), None);
    let b = engine.layout_uncached(&spaced, &ParagraphStyle::rtl(), None);
    println!("plain {} spaced {}", a.natural_width(), b.natural_width());
    for run in b.glyph_runs() {
        println!("run size {} face {:?} glyphs {:?}", run.font_size, run.face, b.glyphs(run).iter().map(|g| (g.id, g.x, g.advance)).collect::<Vec<_>>());
    }
}
