pub(crate) fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n'
            | '\u{0B}'
            | '\u{0C}'
            | '\r'
            | ' '
            | '\u{A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

pub fn js_trim(value: &str) -> &str {
    value.trim_matches(is_js_whitespace)
}

pub(crate) fn collapse_whitespace(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut in_space = false;
    for c in value.chars() {
        if is_js_whitespace(c) {
            if !in_space {
                result.push(' ');
                in_space = true;
            }
        } else {
            result.push(c);
            in_space = false;
        }
    }
    result
}

pub(crate) fn utf16_len(value: &str) -> usize {
    value.chars().map(char::len_utf16).sum()
}

pub(crate) fn utf16_prefix(value: &str, limit: usize) -> String {
    let mut units = 0;
    let mut result = String::new();
    for c in value.chars() {
        let width = c.len_utf16();
        if units + width > limit {
            if units < limit {
                result.push('\u{FFFD}');
            }
            break;
        }
        units += width;
        result.push(c);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_follows_ecmascript() {
        assert_eq!(js_trim("\u{FEFF} a \u{85}"), "a \u{85}");
        assert_eq!(collapse_whitespace("a \t\n b\u{3000}c"), "a b c");
    }

    #[test]
    fn utf16_prefix_replaces_split_surrogates() {
        assert_eq!(utf16_prefix("ab😀c", 3), "ab\u{FFFD}");
        assert_eq!(utf16_prefix("ab😀c", 4), "ab😀");
        assert_eq!(utf16_prefix("ab", 280), "ab");
    }
}
