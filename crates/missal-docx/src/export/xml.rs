pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if allowed(c) => out.push(c),
            _ => {}
        }
    }
    out
}

fn allowed(c: char) -> bool {
    matches!(c as u32, 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)
}

pub fn twips(px: f64) -> i64 {
    let value = (px * 15.0).round();
    if value.is_finite() {
        value.clamp(-31_680.0, 31_680.0) as i64
    } else {
        0
    }
}

pub fn eighths(px: f64) -> i64 {
    (px * 6.0).round().clamp(2.0, 96.0) as i64
}
