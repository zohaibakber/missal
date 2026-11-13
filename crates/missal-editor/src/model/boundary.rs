use missal_page::OBJECT;
use unicode_segmentation::UnicodeSegmentation;

fn floor(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

pub fn is_arabic(c: char) -> bool {
    matches!(c as u32,
        0x0600..=0x06FF | 0x0750..=0x077F | 0x0870..=0x089F | 0x08A0..=0x08FF
        | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF | 0x10E60..=0x10E7F | 0x1EE00..=0x1EEFF)
}

pub fn previous_delete(text: &str, offset: usize) -> usize {
    let offset = floor(text, offset);
    let head = &text[..offset];
    let Some(last) = head.chars().next_back() else {
        return 0;
    };
    if last == OBJECT {
        return offset - last.len_utf8();
    }
    let grapheme = head.graphemes(true).next_back().unwrap_or("");
    if grapheme.chars().any(is_arabic) {
        return offset - last.len_utf8();
    }
    offset - grapheme.len()
}

pub fn next_delete(text: &str, offset: usize) -> usize {
    let offset = floor(text, offset);
    let rest = &text[offset..];
    match rest.graphemes(true).next() {
        Some(grapheme) if grapheme.starts_with(OBJECT) => offset + OBJECT.len_utf8(),
        Some(grapheme) => offset + grapheme.len(),
        None => offset,
    }
}

pub fn previous_word(text: &str, offset: usize) -> usize {
    missal_text::previous_word_start(text, offset)
}

pub fn next_word(text: &str, offset: usize) -> usize {
    missal_text::next_word_end(text, offset)
}

pub fn word_at(text: &str, offset: usize) -> std::ops::Range<usize> {
    missal_text::word_range_at(text, offset)
}
