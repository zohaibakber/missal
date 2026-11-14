use std::ops::Range;

use missal_core::document::{Field, Inline, Link, Style, Text, TextFormat};
use missal_page::OBJECT_LEN;

pub fn inline_len(inline: &Inline) -> usize {
    match inline {
        Inline::Text(text) => text.text.len(),
        Inline::LineBreak(_) => 1,
        Inline::Field(_) | Inline::Image(_) => OBJECT_LEN,
        Inline::Link(link) => len(&link.children),
    }
}

pub fn len(inlines: &[Inline]) -> usize {
    inlines.iter().map(inline_len).sum()
}

pub fn text_of(inlines: &[Inline]) -> String {
    missal_page::logical_text(inlines)
}

fn floor_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn tail_text(text: &Text, rest: String) -> Text {
    Text {
        text: rest,
        format: text.format,
        style: text.style.clone(),
        imported_style: text.imported_style.clone(),
    }
}

pub fn split_off(inlines: &mut Vec<Inline>, offset: usize) -> Vec<Inline> {
    let mut acc = 0;
    let mut i = 0;
    while i < inlines.len() {
        let l = inline_len(&inlines[i]);
        if l > 0 && offset <= acc {
            break;
        }
        if offset < acc + l {
            match &mut inlines[i] {
                Inline::Text(text) => {
                    let k = floor_boundary(&text.text, offset - acc);
                    if k == 0 {
                        break;
                    }
                    let rest = text.text.split_off(k);
                    let piece = tail_text(text, rest);
                    let mut tail = inlines.split_off(i + 1);
                    tail.insert(0, Inline::Text(piece));
                    return tail;
                }
                Inline::Link(link) => {
                    let inner = split_off(&mut link.children, offset - acc);
                    let piece = Link {
                        props: link.props.clone(),
                        url: link.url.clone(),
                        rel: link.rel.clone(),
                        target: link.target.clone(),
                        title: link.title.clone(),
                        children: inner,
                    };
                    let mut tail = inlines.split_off(i + 1);
                    tail.insert(0, Inline::Link(piece));
                    return tail;
                }
                _ => break,
            }
        }
        acc += l;
        i += 1;
    }
    inlines.split_off(i)
}

pub fn snap(inlines: &[Inline], offset: usize) -> usize {
    let mut acc = 0;
    for inline in inlines {
        let l = inline_len(inline);
        if offset < acc + l && offset > acc {
            return match inline {
                Inline::Text(text) => acc + floor_boundary(&text.text, offset - acc),
                Inline::Link(link) => acc + snap(&link.children, offset - acc),
                _ => acc,
            };
        }
        acc += l;
    }
    offset.min(acc)
}

pub fn delete(inlines: &mut Vec<Inline>, range: Range<usize>) {
    if range.start >= range.end {
        return;
    }
    let tail = split_off(inlines, range.end);
    let _ = split_off(inlines, range.start);
    inlines.extend(tail);
    normalize(inlines);
}

pub fn slice(inlines: &[Inline], range: Range<usize>) -> Vec<Inline> {
    let mut copy = inlines.to_vec();
    let _ = split_off(&mut copy, range.end);
    split_off(&mut copy, range.start)
}

pub fn insert(inlines: &mut Vec<Inline>, offset: usize, new: Vec<Inline>) {
    let tail = split_off(inlines, offset);
    inlines.extend(new);
    inlines.extend(tail);
    normalize(inlines);
}

pub fn same_attrs(a: &Text, b: &Text) -> bool {
    a.format == b.format && a.style == b.style && a.imported_style == b.imported_style
}

pub fn normalize(inlines: &mut Vec<Inline>) {
    let mut out: Vec<Inline> = Vec::with_capacity(inlines.len());
    for inline in inlines.drain(..) {
        match inline {
            Inline::Text(text) if text.text.is_empty() => {}
            Inline::Text(text) => {
                if let Some(Inline::Text(previous)) = out.last_mut()
                    && same_attrs(previous, &text)
                {
                    previous.text.push_str(&text.text);
                    continue;
                }
                out.push(Inline::Text(text));
            }
            Inline::Link(mut link) => {
                normalize(&mut link.children);
                if !link.children.is_empty() {
                    out.push(Inline::Link(link));
                }
            }
            other => out.push(other),
        }
    }
    *inlines = out;
}

pub fn template_at(inlines: &[Inline], offset: usize) -> Option<Text> {
    let mut acc = 0;
    let mut before: Option<&Text> = None;
    let mut after: Option<&Text> = None;
    for inline in inlines {
        let l = inline_len(inline);
        match inline {
            Inline::Link(link) if acc < offset && offset < acc + l => {
                if let Some(found) = template_at(&link.children, offset - acc) {
                    return Some(found);
                }
            }
            Inline::Text(text) => {
                if acc < offset {
                    before = Some(text);
                } else if after.is_none() {
                    after = Some(text);
                }
            }
            _ => {}
        }
        acc += l;
    }
    before.or(after).map(|text| tail_text(text, String::new()))
}

pub fn new_text(text: &str, format: TextFormat, style: Option<Style>) -> Text {
    Text {
        text: text.to_owned(),
        format,
        style: style.filter(|style| !style.is_empty()),
        imported_style: None,
    }
}

pub fn insert_text(inlines: &mut Vec<Inline>, offset: usize, insert: &str, template: &Text) {
    if insert.is_empty() {
        return;
    }
    if insert_into_run(inlines, offset, insert, template) {
        return;
    }
    let run = tail_text(template, insert.to_owned());
    let tail = split_off(inlines, offset);
    inlines.push(Inline::Text(run));
    inlines.extend(tail);
    normalize(inlines);
}

fn insert_into_run(inlines: &mut [Inline], offset: usize, insert: &str, template: &Text) -> bool {
    let mut acc = 0;
    for inline in inlines.iter_mut() {
        let l = inline_len(inline);
        match inline {
            Inline::Link(link) if acc < offset && offset < acc + l => {
                if insert_into_run(&mut link.children, offset - acc, insert, template) {
                    return true;
                }
                let run = tail_text(template, insert.to_owned());
                let tail = split_off(&mut link.children, offset - acc);
                link.children.push(Inline::Text(run));
                link.children.extend(tail);
                normalize(&mut link.children);
                return true;
            }
            Inline::Text(text)
                if ((acc < offset && offset <= acc + l) || (offset == 0 && acc == 0))
                    && same_attrs(text, template) =>
            {
                let at = floor_boundary(&text.text, offset - acc);
                text.text.insert_str(at, insert);
                return true;
            }
            _ => {}
        }
        acc += l;
        if acc > offset {
            return false;
        }
    }
    false
}

pub fn object_at(inlines: &[Inline], offset: usize) -> Option<&Inline> {
    let mut acc = 0;
    for inline in inlines {
        let l = inline_len(inline);
        if acc == offset && matches!(inline, Inline::Field(_) | Inline::Image(_)) {
            return Some(inline);
        }
        if let Inline::Link(link) = inline
            && acc <= offset
            && offset < acc + l
        {
            return object_at(&link.children, offset - acc);
        }
        acc += l;
        if acc > offset {
            break;
        }
    }
    None
}

pub fn field_at(inlines: &[Inline], offset: usize) -> Option<&Field> {
    match object_at(inlines, offset)? {
        Inline::Field(field) => Some(field),
        _ => None,
    }
}

pub fn for_each_text_in(inlines: &[Inline], range: Range<usize>, f: &mut dyn FnMut(&Text)) {
    let mut acc = 0;
    for inline in inlines {
        let l = inline_len(inline);
        let overlaps = acc < range.end && range.start < acc + l;
        if overlaps {
            match inline {
                Inline::Text(text) => f(text),
                Inline::Link(link) => for_each_text_in(
                    &link.children,
                    range.start.saturating_sub(acc)..(range.end - acc).min(l),
                    f,
                ),
                _ => {}
            }
        }
        acc += l;
        if acc >= range.end {
            break;
        }
    }
}

fn map_texts(inlines: &mut [Inline], f: &mut dyn FnMut(&mut Text)) {
    for inline in inlines {
        match inline {
            Inline::Text(text) => f(text),
            Inline::Link(link) => map_texts(&mut link.children, f),
            _ => {}
        }
    }
}

pub fn map_range(inlines: &mut Vec<Inline>, range: Range<usize>, f: &mut dyn FnMut(&mut Text)) {
    if range.start >= range.end {
        return;
    }
    let tail = split_off(inlines, range.end);
    let mut middle = split_off(inlines, range.start);
    map_texts(&mut middle, f);
    inlines.extend(middle);
    inlines.extend(tail);
    normalize(inlines);
}

pub const FLAGS: [TextFormat; 3] = [TextFormat::BOLD, TextFormat::ITALIC, TextFormat::UNDERLINE];

fn style_has(style: Option<&Style>, flag: TextFormat) -> bool {
    let Some(style) = style else { return false };
    if flag == TextFormat::BOLD {
        return style.get("font-weight").is_some_and(|value| {
            let value = value.trim().to_ascii_lowercase();
            value == "bold"
                || value == "bolder"
                || value.parse::<f32>().is_ok_and(|weight| weight >= 600.0)
        });
    }
    if flag == TextFormat::ITALIC {
        return style.get("font-style").is_some_and(|value| {
            let value = value.trim().to_ascii_lowercase();
            value.starts_with("italic") || value.starts_with("oblique")
        });
    }
    if flag == TextFormat::UNDERLINE {
        return ["text-decoration", "text-decoration-line"]
            .iter()
            .any(|property| {
                style
                    .get(property)
                    .is_some_and(|value| value.to_ascii_lowercase().contains("underline"))
            });
    }
    false
}

pub fn has_flag(text: &Text, flag: TextFormat) -> bool {
    text.format().contains(flag)
        || style_has(text.style.as_ref(), flag)
        || style_has(text.imported_style.as_ref(), flag)
}

fn strip_style(style: &Style, flag: TextFormat) -> Style {
    let mut parts: Vec<String> = Vec::new();
    for declaration in style.as_str().split(';') {
        let Some((property, value)) = declaration.split_once(':') else {
            if !declaration.trim().is_empty() {
                parts.push(declaration.trim().to_owned());
            }
            continue;
        };
        let name = property.trim().to_ascii_lowercase();
        let lowered = value.trim().to_ascii_lowercase();
        let drop = match flag {
            f if f == TextFormat::BOLD => name == "font-weight",
            f if f == TextFormat::ITALIC => {
                name == "font-style"
                    && (lowered.starts_with("italic") || lowered.starts_with("oblique"))
            }
            _ => false,
        };
        if drop {
            continue;
        }
        if flag == TextFormat::UNDERLINE
            && (name == "text-decoration" || name == "text-decoration-line")
            && lowered.contains("underline")
        {
            let rest: Vec<&str> = value
                .split_whitespace()
                .filter(|token| !token.eq_ignore_ascii_case("underline"))
                .collect();
            if rest.is_empty() || rest.iter().all(|t| t.eq_ignore_ascii_case("none")) {
                continue;
            }
            parts.push(format!("{}: {}", property.trim(), rest.join(" ")));
            continue;
        }
        parts.push(format!("{}: {}", property.trim(), value.trim()));
    }
    Style::new(&parts.join("; "))
}

fn stripped(style: &Option<Style>, flag: TextFormat) -> Option<Style> {
    style
        .as_ref()
        .map(|style| {
            if style_has(Some(style), flag) {
                strip_style(style, flag)
            } else {
                style.clone()
            }
        })
        .filter(|style| !style.is_empty())
}

pub fn set_flag_on(text: &mut Text, flag: TextFormat, on: bool) {
    text.format.set(flag, on);
    if !on {
        text.style = stripped(&text.style, flag);
        text.imported_style = stripped(&text.imported_style, flag);
    }
}

pub fn css_for_field(text: &Text) -> Option<Style> {
    let mut parts: Vec<String> = Vec::new();
    for style in [text.imported_style.as_ref(), text.style.as_ref()]
        .into_iter()
        .flatten()
    {
        let css = style.as_str().trim().trim_end_matches(';').trim();
        if !css.is_empty() {
            parts.push(css.to_owned());
        }
    }
    let format = text.format();
    if format.contains(TextFormat::BOLD) {
        parts.push("font-weight: 700".into());
    }
    if format.contains(TextFormat::ITALIC) {
        parts.push("font-style: italic".into());
    }
    let mut decorations = Vec::new();
    if format.contains(TextFormat::UNDERLINE) {
        decorations.push("underline");
    }
    if format.contains(TextFormat::STRIKETHROUGH) {
        decorations.push("line-through");
    }
    if !decorations.is_empty() {
        parts.push(format!("text-decoration: {}", decorations.join(" ")));
    }
    (!parts.is_empty()).then(|| Style::new(&parts.join("; ")))
}

pub fn strip_objects(text: &str) -> String {
    text.chars()
        .filter(|&c| {
            c != missal_page::OBJECT
                && (!c.is_control() || c == '\t' || c == '\n')
                && !matches!(c, '\u{2028}' | '\u{2029}')
        })
        .collect()
}
