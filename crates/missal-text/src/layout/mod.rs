mod engine;
mod navigation;
mod style;
mod text_layout;

pub use engine::{CacheStats, TextEngine};
pub use navigation::{
    HorizontalDirection, next_cluster_boundary, next_word_end, next_word_start,
    paragraph_range_at, previous_char_boundary, previous_word_start, word_range_at,
};
pub use style::{
    Align, Attr, AttributedText, Direction, InlineObject, LineHeight, ParagraphStyle, Rgba,
    TextStyle, first_strong_is_rtl,
};
pub use text_layout::{
    Affinity, Background, Caret, Decoration, DecorationKind, GlyphRun, LineInfo, PlacedObject,
    PositionedGlyph, Rect, Synthesis, TextLayout, TextPosition,
};
