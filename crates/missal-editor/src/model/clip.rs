use missal_core::document::{Block, Document, Inline, List, ListItemChild, Paragraph, Text};
use missal_core::{FieldDisplayText, FieldReference};
use missal_page::{BlockPath, Position};

use super::edit::{self, insert_inlines, split};
use super::inline::{self, len};
use super::selection::{block_len, pos};
use super::tree::{self, HostKind, anatomy, container, container_mut, host_kind, inlines};

pub const FRAGMENT_TAG: &str = "missal-fragment";

#[derive(Clone, Debug, PartialEq)]
pub struct Fragment {
    pub blocks: Vec<Block>,
    pub inline: bool,
}

impl Fragment {
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn encode(&self) -> String {
        let document = Document::new(self.blocks.clone());
        format!(
            "{FRAGMENT_TAG}:{}:{}",
            u8::from(self.inline),
            missal_core::text::encode_base64(&document.encode())
        )
    }

    pub fn decode(raw: &str) -> Option<Fragment> {
        let rest = raw.strip_prefix(FRAGMENT_TAG)?.strip_prefix(':')?;
        let (inline, data) = rest.split_once(':')?;
        let document = Document::decode(&missal_core::text::decode_base64(data)?).ok()?;
        Some(Fragment {
            blocks: document.blocks,
            inline: inline == "1",
        })
    }
}

fn sliced_block(block: &Block, children: Vec<Inline>) -> Block {
    match block {
        Block::Heading(h) => Block::Heading(missal_core::document::Heading {
            props: h.props.clone(),
            level: h.level,
            children,
        }),
        Block::Quote(q) => Block::Quote(missal_core::document::Quote {
            props: q.props.clone(),
            children,
        }),
        Block::Paragraph(p) => Block::Paragraph(Paragraph {
            props: p.props.clone(),
            children,
        }),
        _ => Block::Paragraph(Paragraph::new(children)),
    }
}

fn host_block(doc: &Document, path: &BlockPath, range: std::ops::Range<usize>) -> Option<Block> {
    let found = anatomy(doc, path)?;
    let block = container(doc, &found.container)?.get(found.block)?;
    let children = inline::slice(&inlines(doc, path)?, range);
    if found.tail.is_empty() {
        return Some(sliced_block(block, children));
    }
    let Block::List(list) = block else {
        return None;
    };
    let item = tree::item(list, &found.tail)?;
    Some(Block::Paragraph(Paragraph {
        props: item.props.clone(),
        children,
    }))
}

pub fn copy(doc: &Document, s: &Position, e: &Position) -> Fragment {
    if s.path == e.path {
        let children = inlines(doc, &s.path)
            .map(|found| inline::slice(&found, s.offset..e.offset))
            .unwrap_or_default();
        return Fragment {
            blocks: vec![Block::Paragraph(Paragraph::new(children))],
            inline: true,
        };
    }
    let mut blocks = Vec::new();
    let last = e.path.top_index().min(doc.blocks.len().saturating_sub(1));
    for k in s.path.top_index()..=last {
        let block = &doc.blocks[k];
        let path = BlockPath::top(k);
        let touches = s.path.starts_with(path.steps()) || e.path.starts_with(path.steps());
        let strictly_between = path > s.path && path < e.path && !touches;
        if strictly_between {
            blocks.push(block.clone());
            continue;
        }
        for host in tree::text_paths_of_block(block, path.clone()) {
            if host < s.path || host > e.path {
                continue;
            }
            let length = block_len(doc, &host).unwrap_or(0);
            let from = if host == s.path { s.offset } else { 0 };
            let to = if host == e.path { e.offset } else { length };
            if let Some(sliced) = host_block(doc, &host, from..to.max(from)) {
                blocks.push(sliced);
            }
        }
    }
    Fragment {
        blocks,
        inline: false,
    }
}

pub fn plain_text(
    fragment: &Fragment,
    display: &dyn Fn(&FieldReference) -> FieldDisplayText,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    for block in &fragment.blocks {
        block_text(block, display, &mut lines);
    }
    lines.join("\n")
}

fn inline_text(
    inlines: &[Inline],
    display: &dyn Fn(&FieldReference) -> FieldDisplayText,
    out: &mut String,
) {
    for inline in inlines {
        match inline {
            Inline::Text(text) => out.push_str(&text.text),
            Inline::LineBreak(_) => out.push('\n'),
            Inline::Field(field) => out.push_str(&display(&field.reference).text),
            Inline::Link(link) => inline_text(&link.children, display, out),
            Inline::Image(_) => {}
        }
    }
}

fn block_text(
    block: &Block,
    display: &dyn Fn(&FieldReference) -> FieldDisplayText,
    lines: &mut Vec<String>,
) {
    match block {
        Block::Paragraph(p) => {
            let mut line = String::new();
            inline_text(&p.children, display, &mut line);
            lines.push(line);
        }
        Block::Heading(h) => {
            let mut line = String::new();
            inline_text(&h.children, display, &mut line);
            lines.push(line);
        }
        Block::Quote(q) => {
            let mut line = String::new();
            inline_text(&q.children, display, &mut line);
            lines.push(line);
        }
        Block::List(list) => list_text(list, display, lines),
        Block::Table(table) => {
            for row in table.rows.iter() {
                let mut cells = Vec::new();
                for cell in row.cells.iter() {
                    let mut inner = Vec::new();
                    for block in &cell.blocks {
                        block_text(block, display, &mut inner);
                    }
                    cells.push(inner.join(" "));
                }
                lines.push(cells.join("\t"));
            }
        }
        _ => {}
    }
}

fn list_text(
    list: &List,
    display: &dyn Fn(&FieldReference) -> FieldDisplayText,
    lines: &mut Vec<String>,
) {
    for item in list.items.iter() {
        let mut line = String::new();
        let mut has_inline = false;
        for child in &item.children {
            match child {
                ListItemChild::Inline(inline) => {
                    has_inline = true;
                    inline_text(std::slice::from_ref(inline), display, &mut line);
                }
                ListItemChild::List(nested) => list_text(nested, display, lines),
            }
        }
        if has_inline || item.children.is_empty() {
            lines.push(line);
        }
    }
}

pub fn from_text(text: &str, template: &Text) -> Fragment {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let clean = inline::strip_objects(&text);
    let lines: Vec<&str> = clean.split('\n').collect();
    let blocks = lines
        .iter()
        .map(|line| {
            let children = if line.is_empty() {
                Vec::new()
            } else {
                vec![Inline::Text(Text {
                    text: (*line).to_owned(),
                    ..template.clone()
                })]
            };
            Block::Paragraph(Paragraph::new(children))
        })
        .collect();
    Fragment {
        blocks,
        inline: lines.len() == 1,
    }
}

pub fn looks_like_html(text: &str) -> bool {
    let head: String = text
        .trim_start()
        .chars()
        .take(64)
        .collect::<String>()
        .to_ascii_lowercase();
    head.starts_with("<!doctype html")
        || head.starts_with("<html")
        || head.starts_with("<meta")
        || (head.starts_with('<')
            && text.contains("</")
            && (text.contains("<p")
                || text.contains("<div")
                || text.contains("<span")
                || text.contains("<br")))
}

pub fn html_to_text(html: &str) -> String {
    let mut out = String::new();
    let mut chars = html.chars().peekable();
    let mut skip_depth: Option<String> = None;
    while let Some(c) = chars.next() {
        if c == '<' {
            let mut tag = String::new();
            for t in chars.by_ref() {
                if t == '>' {
                    break;
                }
                tag.push(t);
            }
            let lower = tag.trim().to_ascii_lowercase();
            let name: String = lower
                .trim_start_matches('/')
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            let closing = lower.starts_with('/');
            if let Some(skipped) = &skip_depth {
                if closing && &name == skipped {
                    skip_depth = None;
                }
                continue;
            }
            if !closing && matches!(name.as_str(), "script" | "style" | "head" | "title") {
                skip_depth = Some(name);
                continue;
            }
            match name.as_str() {
                "br" => out.push('\n'),
                "p" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                | "blockquote" | "table" => {
                    if !out.is_empty() && !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
                "td" | "th" if closing => out.push('\t'),
                _ => {}
            }
            continue;
        }
        if skip_depth.is_some() {
            continue;
        }
        if c == '&' {
            let mut entity = String::new();
            while let Some(&n) = chars.peek() {
                if n == ';' || entity.len() > 8 {
                    break;
                }
                entity.push(n);
                chars.next();
            }
            if chars.peek() == Some(&';') {
                chars.next();
            }
            out.push_str(&decode_entity(&entity));
            continue;
        }
        if c == '\n' || c == '\r' {
            out.push(' ');
            continue;
        }
        out.push(c);
    }
    out.lines()
        .map(|line| line.trim_matches(|c: char| c == ' ' || c == '\t'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches('\n')
        .to_owned()
}

fn decode_entity(entity: &str) -> String {
    match entity {
        "amp" => "&".into(),
        "lt" => "<".into(),
        "gt" => ">".into(),
        "quot" => "\"".into(),
        "apos" => "'".into(),
        "nbsp" => "\u{a0}".into(),
        _ => {
            let code = entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()));
            match code.and_then(char::from_u32) {
                Some(c) => c.to_string(),
                None => format!("&{entity};"),
            }
        }
    }
}

fn inline_children(block: Block) -> Option<Vec<Inline>> {
    match block {
        Block::Paragraph(p) => Some(p.children),
        Block::Heading(h) => Some(h.children),
        Block::Quote(q) => Some(q.children),
        _ => None,
    }
}

fn flatten_to_paragraphs(blocks: Vec<Block>, out: &mut Vec<Vec<Inline>>) {
    for block in blocks {
        match block {
            Block::Paragraph(_) | Block::Heading(_) | Block::Quote(_) => {
                out.push(inline_children(block).unwrap_or_default())
            }
            Block::List(list) => {
                for item in list.items {
                    let mut line = Vec::new();
                    let mut nested = Vec::new();
                    for child in item.children {
                        match child {
                            ListItemChild::Inline(inline) => line.push(inline),
                            ListItemChild::List(list) => nested.push(Block::List(list)),
                        }
                    }
                    out.push(line);
                    flatten_to_paragraphs(nested, out);
                }
            }
            Block::Table(table) => {
                for row in table.rows {
                    for cell in row.cells {
                        flatten_to_paragraphs(cell.blocks, out);
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn paste(doc: &mut Document, at: &Position, fragment: Fragment) -> Position {
    if fragment.blocks.is_empty() {
        return at.clone();
    }
    let in_item = host_kind(doc, &at.path) == Some(HostKind::Item);
    if fragment.inline
        || fragment.blocks.len() == 1 && inline_children(fragment.blocks[0].clone()).is_some()
    {
        let children = fragment
            .blocks
            .into_iter()
            .next()
            .and_then(inline_children)
            .unwrap_or_default();
        return insert_inlines(doc, at, children);
    }
    if in_item {
        let mut lines = Vec::new();
        flatten_to_paragraphs(fragment.blocks, &mut lines);
        let mut caret = at.clone();
        for (index, line) in lines.into_iter().enumerate() {
            if index > 0 {
                caret = edit::split(doc, &caret);
            }
            caret = insert_inlines(doc, &caret, line);
        }
        return caret;
    }
    let mut blocks = fragment.blocks;
    let first = blocks.remove(0);
    let last = if blocks.is_empty() {
        None
    } else {
        blocks.pop()
    };
    let after = split(doc, at);
    let Some(found) = anatomy(doc, &at.path) else {
        return after;
    };
    let mut insert_index = found.block + 1;
    let mut caret_path_index = found.block + 1;
    let mut caret_offset = 0;
    match inline_children(first.clone()) {
        Some(children) => {
            insert_inlines(doc, &pos(at.path.clone(), at.offset), children);
        }
        None => {
            if let Some(container) = container_mut(doc, &found.container) {
                container.insert(insert_index, first);
                insert_index += 1;
                caret_path_index += 1;
            }
        }
    }
    let middle_count = blocks.len();
    if let Some(container) = container_mut(doc, &found.container) {
        for (i, block) in blocks.into_iter().enumerate() {
            container.insert(insert_index + i, block);
        }
    }
    caret_path_index += middle_count;
    if let Some(last) = last {
        let mut steps = found.container.clone();
        steps.push(caret_path_index as u32);
        let tail_path = BlockPath(steps);
        match inline_children(last.clone()) {
            Some(children) => {
                caret_offset = len(&children);
                insert_inlines(doc, &pos(tail_path.clone(), 0), children);
            }
            None => {
                if let Some(container) = container_mut(doc, &found.container) {
                    container.insert(caret_path_index, last);
                    caret_path_index += 1;
                }
            }
        }
    }
    let mut steps = found.container.clone();
    steps.push(caret_path_index as u32);
    let caret = pos(BlockPath(steps), caret_offset);
    if inlines(doc, &caret.path).is_some() {
        caret
    } else {
        pos(at.path.clone(), block_len(doc, &at.path).unwrap_or(0))
    }
}
