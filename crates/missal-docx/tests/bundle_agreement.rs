use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use flate2::read::GzDecoder;
use missal_core::document::{
    Align, Block, Child, Direction, Inline, ListItemChild, Paragraph, TextFormat,
};
use missal_core::field::fir_field_label;
use missal_core::{
    Document, FieldCatalog, FieldMarkers, FieldReference, Placeholder, PlaceholderId,
    PlaceholderSource, field_lookup_key,
};
use serde_json::Value;

const TEMPLATES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../templates");

fn bundle() -> Vec<(String, Document)> {
    let bytes = std::fs::read(Path::new(TEMPLATES).join("bundle.json.gz")).unwrap();
    let mut json = String::new();
    GzDecoder::new(bytes.as_slice())
        .read_to_string(&mut json)
        .unwrap();
    let Value::Array(items) = serde_json::from_str(&json).unwrap() else {
        panic!("bundle is not an array");
    };
    items
        .into_iter()
        .map(|item| {
            let name = item["name"].as_str().unwrap().to_owned();
            let envelope =
                missal_core::document::Envelope::from_value(item["document"].clone()).unwrap();
            (name, Document::from_envelope(&envelope).unwrap())
        })
        .collect()
}

fn catalog() -> FieldCatalog {
    let fields: Value = serde_json::from_str(
        &std::fs::read_to_string(Path::new(TEMPLATES).join("fields.json")).unwrap(),
    )
    .unwrap();
    let placeholders = ["global", "custom"]
        .iter()
        .flat_map(|key| fields[key].as_array().unwrap().clone())
        .enumerate()
        .map(|(index, label)| Placeholder {
            id: PlaceholderId::new(index as i64 + 1).unwrap(),
            label: label.as_str().unwrap().to_owned(),
            source: PlaceholderSource::Custom,
        })
        .collect();
    FieldCatalog::new(placeholders)
}

#[derive(Debug, Clone, PartialEq)]
struct Para {
    text: String,
    with_fields: String,
    align: &'static str,
    direction: &'static str,
    flags: String,
    fonts: Vec<String>,
}

#[derive(Default)]
struct Features {
    kinds: Vec<String>,
    paragraphs: Vec<Para>,
    fields: Vec<String>,
    tables: Vec<String>,
    breaks: Vec<String>,
    lists: usize,
}

fn normalize(text: &str) -> String {
    text.replace(['\u{a0}', '\u{2003}', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn label(catalog: &FieldCatalog, reference: &FieldReference) -> String {
    let label = match reference {
        FieldReference::Unresolved(text) => text.clone(),
        FieldReference::Fir(field) => fir_field_label(*field).to_owned(),
        other => catalog.label(other).unwrap_or_default(),
    };
    field_lookup_key(&label)
}

fn style_flags(css: Option<&missal_core::document::Style>) -> u32 {
    let Some(css) = css else { return 0 };
    let mut flags = 0;
    if css
        .get("font-weight")
        .is_some_and(|weight| weight.parse::<u32>().unwrap_or(400) >= 600)
    {
        flags |= 1;
    }
    if css.get("font-style").is_some_and(|style| style == "italic") {
        flags |= 2;
    }
    if css
        .get("text-decoration")
        .is_some_and(|decoration| decoration.contains("underline"))
    {
        flags |= 8;
    }
    flags
}

fn font_key(css: Option<&missal_core::document::Style>) -> String {
    let Some(css) = css else { return String::new() };
    let family = css
        .get("font-family")
        .unwrap_or_default()
        .split(',')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches('"')
        .to_owned();
    let size = css
        .get("font-size")
        .and_then(|size| size.trim_end_matches("px").parse::<f64>().ok())
        .unwrap_or(0.0);
    format!("{family}/{size:.1}")
}

fn flag_char(flags: u32) -> char {
    char::from(b'a' + (flags & 0b1011) as u8)
}

fn visual_align(align: Option<Align>, rtl: bool) -> &'static str {
    match align {
        Some(Align::Center) => "center",
        Some(Align::Justify) => "justify",
        Some(Align::Left) => "left",
        Some(Align::Right) => "right",
        Some(Align::End) => {
            if rtl {
                "left"
            } else {
                "right"
            }
        }
        Some(Align::Start) | None => {
            if rtl {
                "right"
            } else {
                "left"
            }
        }
    }
}

struct Collector<'c> {
    catalog: &'c FieldCatalog,
    features: Features,
}

impl Collector<'_> {
    fn inlines(&mut self, inlines: &[Inline], para: &mut Para) {
        for inline in inlines {
            match inline {
                Inline::Text(text) => {
                    para.text.push_str(&text.text);
                    para.with_fields.push_str(&text.text);
                    let flags =
                        text.format.unwrap_or(TextFormat(0)).0 | style_flags(text.style.as_ref());
                    let font = font_key(text.style.as_ref());
                    for c in text.text.chars() {
                        if !normalize(&c.to_string()).is_empty() {
                            para.flags.push(flag_char(flags));
                            para.fonts.push(font.clone());
                        }
                    }
                }
                Inline::Field(field) => {
                    let name = label(self.catalog, &field.reference);
                    para.with_fields.push_str(&format!(" «{name}» "));
                    para.flags
                        .push(flag_char(style_flags(field.imported_style.as_ref())));
                    para.fonts.push(font_key(field.imported_style.as_ref()));
                    self.features.fields.push(name);
                }
                Inline::LineBreak(_) => {
                    para.text.push(' ');
                    para.with_fields.push(' ');
                }
                Inline::Link(link) => self.inlines(&link.children, para),
                _ => {}
            }
        }
    }

    fn paragraph(
        &mut self,
        props: &missal_core::document::ElementProps,
        children: &[Inline],
    ) -> Para {
        let rtl = props.direction == Some(Direction::Rtl);
        let mut para = Para {
            text: String::new(),
            with_fields: String::new(),
            align: visual_align(props.align, rtl),
            direction: match props.direction {
                Some(Direction::Rtl) => "rtl",
                Some(Direction::Ltr) => "ltr",
                None => "none",
            },
            flags: String::new(),
            fonts: Vec::new(),
        };
        self.inlines(children, &mut para);
        para.text = normalize(&para.text);
        para.with_fields = normalize(&para.with_fields);
        self.features.paragraphs.push(para.clone());
        para
    }

    fn blocks(&mut self, blocks: &[Block], top: bool) -> Option<String> {
        let mut first = None;
        let mut content: Vec<Option<String>> = Vec::new();
        for block in blocks {
            let fingerprint = match block {
                Block::Paragraph(Paragraph { props, children }) => {
                    if top {
                        self.features.kinds.push("p".into());
                    }
                    let para = self.paragraph(props, children);
                    (!para.with_fields.is_empty()).then_some(para.with_fields)
                }
                Block::List(list) => {
                    self.features.lists += 1;
                    self.list(list, top)
                }
                Block::Table(table) => {
                    if top {
                        self.features.kinds.push("t".into());
                    }
                    let mut shape = String::new();
                    let mut first_text = None;
                    for row in &table.rows {
                        let Child::Node(row) = row else { continue };
                        shape.push('[');
                        for cell in &row.cells {
                            let Child::Node(cell) = cell else { continue };
                            shape.push_str(&format!("{}x{} ", cell.col_span(), cell.row_span()));
                            let text = self.blocks(&cell.blocks, false);
                            if first_text.is_none() {
                                first_text = text;
                            }
                        }
                        shape.push(']');
                    }
                    self.features.tables.push(shape);
                    Some(format!("T:{}", first_text.unwrap_or_default()))
                }
                Block::PageBreak(_) => {
                    if top {
                        self.features.kinds.push("b".into());
                        content.push(None);
                    }
                    continue;
                }
                other => {
                    if top {
                        self.features
                            .kinds
                            .push(format!("{other:?}").chars().take(8).collect());
                    }
                    None
                }
            };
            if first.is_none() {
                first = fingerprint.clone();
            }
            if top && fingerprint.is_some() {
                content.push(fingerprint);
            }
        }
        if top {
            for (index, item) in content.iter().enumerate() {
                if item.is_none() {
                    let before = content[..index]
                        .iter()
                        .rev()
                        .flatten()
                        .next()
                        .cloned()
                        .unwrap_or_default();
                    let after = content[index + 1..]
                        .iter()
                        .flatten()
                        .next()
                        .cloned()
                        .unwrap_or_default();
                    let short = |text: String| text.chars().take(40).collect::<String>();
                    self.features
                        .breaks
                        .push(format!("{} | {}", short(before), short(after)));
                }
            }
        }
        first
    }

    fn list(&mut self, list: &missal_core::document::List, top: bool) -> Option<String> {
        let mut first = None;
        for item in &list.items {
            let Child::Node(item) = item else { continue };
            let inlines: Vec<Inline> = item
                .children
                .iter()
                .filter_map(|child| match child {
                    ListItemChild::Inline(inline) => Some(inline.clone()),
                    ListItemChild::List(_) => None,
                })
                .collect();
            let nested: Vec<_> = item
                .children
                .iter()
                .filter_map(|child| match child {
                    ListItemChild::List(list) => Some(list),
                    ListItemChild::Inline(_) => None,
                })
                .collect();
            if nested.is_empty() || !inlines.is_empty() {
                if top {
                    self.features.kinds.push("p".into());
                }
                let para = self.paragraph(&item.props, &inlines);
                if first.is_none() && !para.with_fields.is_empty() {
                    first = Some(para.with_fields);
                }
            }
            for list in nested {
                let found = self.list(list, top);
                if first.is_none() {
                    first = found;
                }
            }
        }
        first
    }
}

fn features(document: &Document, catalog: &FieldCatalog) -> Features {
    let mut collector = Collector {
        catalog,
        features: Features::default(),
    };
    collector.blocks(&document.blocks, true);
    collector.features
}

fn lcs<T: PartialEq>(a: &[T], b: &[T]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len() as isize, b.len() as isize);
    let mut trace: Vec<Vec<isize>> = Vec::new();
    let mut previous: Vec<isize> = Vec::new();
    let get = |vector: &Vec<isize>, d: isize, k: isize| vector[(k + d) as usize];
    let mut finished = None;
    for d in 0..=(n + m) {
        let mut current = vec![0isize; (2 * d + 1) as usize];
        let mut k = -d;
        while k <= d {
            let mut x = if d == 0 {
                0
            } else if k == -d
                || (k != d && get(&previous, d - 1, k - 1) < get(&previous, d - 1, k + 1))
            {
                get(&previous, d - 1, k + 1)
            } else {
                get(&previous, d - 1, k - 1) + 1
            };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            current[(k + d) as usize] = x;
            if x >= n && y >= m {
                finished = Some(d);
                break;
            }
            k += 2;
        }
        trace.push(current.clone());
        previous = current;
        if finished.is_some() {
            break;
        }
    }
    let mut pairs = Vec::new();
    let (mut x, mut y) = (n, m);
    for d in (0..=finished.unwrap_or(0)).rev() {
        let k = x - y;
        if d == 0 {
            while x > 0 && y > 0 {
                pairs.push(((x - 1) as usize, (y - 1) as usize));
                x -= 1;
                y -= 1;
            }
            break;
        }
        let prior = &trace[(d - 1) as usize];
        let previous_k =
            if k == -d || (k != d && get(prior, d - 1, k - 1) < get(prior, d - 1, k + 1)) {
                k + 1
            } else {
                k - 1
            };
        let previous_x = get(prior, d - 1, previous_k);
        let previous_y = previous_x - previous_k;
        while x > previous_x && y > previous_y {
            pairs.push(((x - 1) as usize, (y - 1) as usize));
            x -= 1;
            y -= 1;
        }
        x = previous_x;
        y = previous_y;
    }
    pairs.reverse();
    pairs
}

#[derive(Default, Clone, Copy)]
struct Score {
    agree: usize,
    total: usize,
}

impl Score {
    fn add(&mut self, other: Score) {
        self.agree += other.agree;
        self.total += other.total;
    }

    fn sequence<T: PartialEq>(mine: &[T], theirs: &[T]) -> Score {
        Score {
            agree: lcs(mine, theirs).len(),
            total: mine.len().max(theirs.len()),
        }
    }

    fn percent(self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            self.agree as f64 * 100.0 / self.total as f64
        }
    }

    fn exact(self) -> bool {
        self.agree == self.total
    }
}

const DIMENSIONS: [&str; 11] = [
    "blocks",
    "text",
    "fields",
    "field-pos",
    "tables",
    "breaks",
    "align",
    "direction",
    "biu",
    "font",
    "layout",
];

fn compare(
    name: &str,
    mine: &Document,
    theirs: &Document,
    catalog: &FieldCatalog,
    verbose: bool,
) -> BTreeMap<&'static str, Score> {
    let a = features(mine, catalog);
    let b = features(theirs, catalog);
    let mut scores = BTreeMap::new();
    scores.insert("blocks", Score::sequence(&a.kinds, &b.kinds));
    let texts = |features: &Features| -> Vec<String> {
        features
            .paragraphs
            .iter()
            .filter(|para| !para.text.is_empty())
            .map(|para| para.text.clone())
            .collect()
    };
    let (text_a, text_b) = (texts(&a), texts(&b));
    scores.insert("text", Score::sequence(&text_a, &text_b));
    scores.insert("fields", Score::sequence(&a.fields, &b.fields));
    let positioned = |features: &Features| -> Vec<String> {
        features
            .paragraphs
            .iter()
            .filter(|para| para.with_fields.contains('«'))
            .map(|para| para.with_fields.clone())
            .collect()
    };
    let (pos_a, pos_b) = (positioned(&a), positioned(&b));
    scores.insert("field-pos", Score::sequence(&pos_a, &pos_b));
    scores.insert("tables", Score::sequence(&a.tables, &b.tables));
    scores.insert("breaks", Score::sequence(&a.breaks, &b.breaks));
    let keyed = |features: &Features| -> Vec<Para> {
        features
            .paragraphs
            .iter()
            .filter(|para| !para.with_fields.is_empty())
            .cloned()
            .collect()
    };
    let (para_a, para_b) = (keyed(&a), keyed(&b));
    let keys_a: Vec<&String> = para_a.iter().map(|para| &para.with_fields).collect();
    let keys_b: Vec<&String> = para_b.iter().map(|para| &para.with_fields).collect();
    let matched = lcs(&keys_a, &keys_b);
    let mut align = Score::default();
    let mut direction = Score::default();
    let mut biu = Score::default();
    let mut font = Score::default();
    for &(i, j) in &matched {
        let (x, y) = (&para_a[i], &para_b[j]);
        align.total += 1;
        direction.total += 1;
        biu.total += 1;
        align.agree += usize::from(x.align == y.align);
        direction.agree += usize::from(x.direction == y.direction);
        biu.agree += usize::from(x.flags == y.flags);
        font.total += 1;
        font.agree += usize::from(x.fonts == y.fonts);
        if verbose && x.fonts != y.fonts {
            let first = x.fonts.iter().zip(&y.fonts).find(|(a, b)| a != b);
            eprintln!(
                "  font mine={:?} bundle={:?} :: {}",
                first.map(|f| f.0),
                first.map(|f| f.1),
                x.with_fields.chars().take(50).collect::<String>()
            );
        }
        if verbose && x.align != y.align {
            eprintln!(
                "  align mine={} bundle={} :: {}",
                x.align,
                y.align,
                x.with_fields.chars().take(60).collect::<String>()
            );
        }
        if verbose && x.direction != y.direction {
            eprintln!(
                "  dir mine={} bundle={} :: {}",
                x.direction,
                y.direction,
                x.with_fields.chars().take(60).collect::<String>()
            );
        }
        if verbose && x.flags != y.flags {
            eprintln!(
                "  biu mine={} bundle={} :: {}",
                x.flags,
                y.flags,
                x.with_fields.chars().take(60).collect::<String>()
            );
        }
    }
    scores.insert("align", align);
    scores.insert("direction", direction);
    scores.insert("biu", biu);
    scores.insert("font", font);
    let layout = |document: &Document| {
        document.page_layout.as_ref().map(|layout| {
            [
                layout.width_mm,
                layout.height_mm,
                layout.margin_top_mm,
                layout.margin_right_mm,
                layout.margin_bottom_mm,
                layout.margin_left_mm,
            ]
        })
    };
    let (layout_a, layout_b) = (layout(mine), layout(theirs));
    let layout_agree = match (layout_a, layout_b) {
        (Some(x), Some(y)) => x
            .iter()
            .zip(y.iter())
            .filter(|(x, y)| (*x - *y).abs() <= 0.1)
            .count(),
        _ => 0,
    };
    scores.insert(
        "layout",
        Score {
            agree: layout_agree,
            total: 6,
        },
    );
    if verbose {
        eprintln!(
            "== {name}: lists={} layout mine={layout_a:?} bundle={layout_b:?}",
            a.lists
        );
        diff("blocks", &a.kinds, &b.kinds);
        diff("text", &text_a, &text_b);
        diff("fields", &a.fields, &b.fields);
        diff("field-pos", &pos_a, &pos_b);
        diff("tables", &a.tables, &b.tables);
        diff("breaks", &a.breaks, &b.breaks);
    }
    scores
}

fn diff<T: PartialEq + std::fmt::Debug>(what: &str, a: &[T], b: &[T]) {
    let pairs = lcs(a, b);
    let mut i = 0;
    let mut j = 0;
    let mut shown = 0;
    for &(x, y) in pairs.iter().chain(std::iter::once(&(a.len(), b.len()))) {
        while i < x {
            if shown < 40 {
                eprintln!("  {what} - mine[{i}] {:?}", a[i]);
            }
            shown += 1;
            i += 1;
        }
        while j < y {
            if shown < 40 {
                eprintln!("  {what} + bundle[{j}] {:?}", b[j]);
            }
            shown += 1;
            j += 1;
        }
        i += 1;
        j += 1;
    }
}

#[test]
fn agrees_with_the_bundled_imports() {
    let catalog = catalog();
    let markers = FieldMarkers::default();
    let verbose = std::env::var("MISSAL_DOCX_DIFF").ok();
    let mut totals: BTreeMap<&str, Score> = BTreeMap::new();
    let mut exact: BTreeMap<&str, usize> = BTreeMap::new();
    let bundle = bundle();
    assert_eq!(bundle.len(), 36);
    eprintln!("{:<52} {}", "template", DIMENSIONS.join(" "));
    for (name, theirs) in &bundle {
        let bytes = std::fs::read(Path::new(TEMPLATES).join(format!("{name}.docx"))).unwrap();
        let imported = missal_docx::import(&bytes, &catalog, &markers).unwrap();
        let show = verbose
            .as_deref()
            .is_some_and(|filter| filter == "all" || name.contains(filter));
        let scores = compare(name, &imported.document, theirs, &catalog, show);
        let line: Vec<String> = DIMENSIONS
            .iter()
            .map(|dimension| format!("{:>5.1}", scores[dimension].percent()))
            .collect();
        eprintln!(
            "{:<52} {}  {:?}",
            name.chars().take(50).collect::<String>(),
            line.join(" "),
            imported.notices
        );
        for (dimension, score) in scores {
            totals.entry(dimension).or_default().add(score);
            *exact.entry(dimension).or_default() += usize::from(score.exact());
        }
    }
    eprintln!("dimension   agreement            templates exact");
    for dimension in DIMENSIONS {
        let score = totals[dimension];
        eprintln!(
            "{dimension:<11} {:>6.2}% ({}/{})  {}/36",
            score.percent(),
            score.agree,
            score.total,
            exact[dimension]
        );
    }
    let floors = [
        ("blocks", 99.5),
        ("text", 99.5),
        ("fields", 100.0),
        ("field-pos", 100.0),
        ("tables", 96.0),
        ("breaks", 100.0),
        ("align", 88.0),
        ("direction", 99.9),
        ("biu", 100.0),
        ("font", 99.9),
        ("layout", 100.0),
    ];
    for (dimension, floor) in floors {
        assert!(
            totals[dimension].percent() >= floor,
            "{dimension} fell below {floor}%"
        );
    }
}
