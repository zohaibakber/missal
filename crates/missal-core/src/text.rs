pub fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_mark(c: char) -> bool {
    matches!(c,
        '\u{0610}'..='\u{061A}'
        | '\u{064B}'..='\u{065F}'
        | '\u{0670}'
        | '\u{06D6}'..='\u{06DC}'
        | '\u{06DF}'..='\u{06E4}'
        | '\u{06E7}'..='\u{06E8}'
        | '\u{06EA}'..='\u{06ED}'
        | '\u{0640}'
        | '\u{200C}'
        | '\u{200D}'
        | '\u{200E}'
        | '\u{200F}'
    )
}

pub fn fold_char(c: char) -> Option<char> {
    if is_mark(c) {
        return None;
    }
    Some(match c {
        '\u{064A}' | '\u{0649}' | '\u{06D0}' => '\u{06CC}',
        '\u{0643}' => '\u{06A9}',
        '\u{0647}' | '\u{06D5}' => '\u{06C1}',
        '\u{06C0}' => '\u{06C2}',
        '\u{0623}' | '\u{0625}' | '\u{0671}' => '\u{0627}',
        '\u{06F0}'..='\u{06F9}' => char::from_u32(c as u32 - 0x06F0 + 0x0030).unwrap_or(c),
        '\u{0660}'..='\u{0669}' => char::from_u32(c as u32 - 0x0660 + 0x0030).unwrap_or(c),
        c if c.is_ascii() => c.to_ascii_lowercase(),
        c => c.to_lowercase().next().unwrap_or(c),
    })
}

pub fn fold_text(text: &str) -> String {
    text.chars().filter_map(fold_char).collect()
}

#[derive(Debug, Clone, Default)]
pub struct Folded {
    text: String,
    origin: Vec<u32>,
}

impl Folded {
    pub fn new(source: &str) -> Folded {
        let mut folded = Folded::default();
        folded.fold(source);
        folded
    }

    pub fn fold(&mut self, source: &str) {
        self.text.clear();
        self.origin.clear();
        for (index, c) in source.char_indices() {
            if let Some(folded) = fold_char(c) {
                self.text.push(folded);
                for _ in 0..folded.len_utf8() {
                    self.origin.push(index as u32);
                }
            }
        }
        self.origin.push(source.len() as u32);
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn origin(&self, folded: usize) -> usize {
        self.origin[folded.min(self.origin.len() - 1)] as usize
    }

    pub fn source_range(&self, folded: std::ops::Range<usize>) -> std::ops::Range<usize> {
        self.origin(folded.start)..self.origin(folded.end)
    }
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode_base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = chunk.iter().enumerate().fold(0u32, |value, (index, byte)| {
            value | u32::from(*byte) << (16 - 8 * index)
        });
        for index in 0..4 {
            if index <= chunk.len() {
                out.push(char::from(
                    BASE64[(value >> (18 - 6 * index) & 63) as usize],
                ));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn base64_value(c: u8) -> Option<u32> {
    Some(match c {
        b'A'..=b'Z' => u32::from(c - b'A'),
        b'a'..=b'z' => u32::from(c - b'a' + 26),
        b'0'..=b'9' => u32::from(c - b'0' + 52),
        b'+' | b'-' => 62,
        b'/' | b'_' => 63,
        _ => return None,
    })
}

pub fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for c in input.bytes() {
        if c == b'=' {
            break;
        }
        if c.is_ascii_whitespace() {
            continue;
        }
        buffer = (buffer << 6) | base64_value(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding_merges_urdu_letter_variants_and_drops_marks() {
        assert_eq!(fold_text("علي"), fold_text("علی"));
        assert_eq!(fold_text("كمال"), fold_text("کمال"));
        assert_eq!(fold_text("مُحمّد"), fold_text("محمد"));
        assert_eq!(fold_text("۳۰۲ Abc"), "302 abc");
    }

    #[test]
    fn folded_offsets_map_back_to_the_source() {
        let source = "مُحمّد علی";
        let folded = Folded::new(source);
        let start = folded.text().find("علی").unwrap();
        let range = folded.source_range(start..start + "علی".len());
        assert_eq!(&source[range], "علی");
        assert_eq!(folded.origin(folded.text().len()), source.len());
    }
}
