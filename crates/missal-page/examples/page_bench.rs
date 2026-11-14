use std::time::Instant;

use missal_core::document::{Block, Inline};
use missal_core::{Document, FieldDisplayText, FieldReference};
use missal_page::pdf::{PdfOptions, write_packet};
use missal_page::{BlockPath, DocSource, Edit, FieldSource, PageEngine, TokenFields};

fn bundle() -> Vec<(String, Document)> {
    missal_store::TemplateBundle::embedded()
        .templates()
        .unwrap()
}

fn text_of(block: &mut Block) -> Option<&mut String> {
    if let Block::Paragraph(p) = block {
        for inline in p.children.iter_mut() {
            if let Inline::Text(t) = inline
                && t.text.chars().count() > 10
            {
                return Some(&mut t.text);
            }
        }
    }
    None
}

fn find_paragraph(doc: &Document, from: usize) -> usize {
    let mut d = doc.clone();
    (from..d.blocks.len())
        .chain(0..from)
        .find(|&i| text_of(&mut d.blocks[i]).is_some())
        .unwrap()
}

fn find_cell_paragraph(doc: &Document, from: usize) -> BlockPath {
    for (i, block) in doc.blocks.iter().enumerate().skip(from) {
        if let Block::Table(t) = block {
            for (r, row) in t.rows.iter().enumerate() {
                let row = row;
                for (c, cell) in row.cells.iter().enumerate() {
                    let cell = cell;
                    for (b, inner) in cell.blocks.iter().enumerate() {
                        if text_of(&mut inner.clone()).is_some() {
                            return BlockPath::new(&[i as u32, r as u32, c as u32, b as u32]);
                        }
                    }
                }
            }
        }
    }
    panic!("no cell paragraph")
}

fn cell_text<'a>(doc: &'a mut Document, path: &BlockPath) -> &'a mut String {
    let s = path.steps();
    let Block::Table(t) = &mut doc.blocks[s[0] as usize] else {
        panic!()
    };
    let row = &mut t.rows[s[1] as usize];
    let cell = &mut row.cells[s[2] as usize];
    text_of(&mut cell.blocks[s[3] as usize]).unwrap()
}

struct Values(bool);

impl FieldSource for Values {
    fn display(&self, reference: &FieldReference) -> FieldDisplayText {
        let base = TokenFields.display(reference);
        if self.0 {
            FieldDisplayText {
                text: format!("{} قدر", base.text),
                unresolved: false,
            }
        } else {
            base
        }
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn main() {
    let all = bundle();
    let (name, doc) = all.iter().max_by_key(|(_, d)| d.encode().len()).unwrap();
    println!(
        "largest template: {name} ({} top-level blocks)",
        doc.blocks.len()
    );
    let fields = TokenFields;
    let started = Instant::now();
    let mut engine = PageEngine::new();
    println!(
        "PageEngine::new (font registration): {:.1} ms",
        started.elapsed().as_secs_f64() * 1e3
    );
    let started = Instant::now();
    let mut packet = engine.layout(&[DocSource::new(doc, &fields)]);
    println!(
        "cold layout: {:.1} ms, {} pages",
        started.elapsed().as_secs_f64() * 1e3,
        packet.page_count()
    );
    let mut colds = Vec::new();
    for _ in 0..5 {
        let mut fresh = PageEngine::new();
        fresh.layout(&[DocSource::new(&all[0].1, &fields)]);
        let started = Instant::now();
        fresh.layout(&[DocSource::new(doc, &fields)]);
        colds.push(started.elapsed().as_secs_f64() * 1e3);
    }
    println!(
        "cold layout (fresh caches, fonts warm), median of 5: {:.1} ms",
        median(colds)
    );
    let started = Instant::now();
    engine.layout(&[DocSource::new(doc, &fields)]);
    println!(
        "warm full relayout (all blocks cached): {:.2} ms",
        started.elapsed().as_secs_f64() * 1e3
    );

    let n = doc.blocks.len();
    for (label, at) in [
        ("start", 0),
        ("middle", n / 2),
        ("end", n.saturating_sub(5)),
    ] {
        let k = find_paragraph(doc, at);
        let mut edited = doc.clone();
        let mut times = Vec::new();
        let mut stats = Vec::new();
        for i in 0..20 {
            let text = text_of(&mut edited.blocks[k]).unwrap();
            let pos = text.char_indices().nth(3).unwrap().0;
            text.insert(pos, if i % 2 == 0 { 'ب' } else { 'ا' });
            let started = Instant::now();
            engine.update(
                &mut packet,
                0,
                &DocSource::new(&edited, &fields),
                &Edit::Block(BlockPath::top(k)),
            );
            times.push(started.elapsed().as_secs_f64() * 1e6);
            stats.push(engine.last_stats());
        }
        let s = stats.last().unwrap();
        println!(
            "one-char edit at {label} (paragraph {k}): median {:.0} us, max {:.0} us; pages rebuilt {} reused {}",
            median(times.clone()),
            times.iter().cloned().fold(0.0, f64::max),
            s.pages_rebuilt,
            s.pages_reused
        );
    }
    for (label, at) in [("start", 0), ("middle", n / 2)] {
        let path = find_cell_paragraph(doc, at);
        let mut edited = doc.clone();
        let mut times = Vec::new();
        for i in 0..20 {
            let text = cell_text(&mut edited, &path);
            let pos = text.char_indices().nth(3).unwrap().0;
            text.insert(pos, if i % 2 == 0 { 'ب' } else { 'ا' });
            let started = Instant::now();
            engine.update(
                &mut packet,
                0,
                &DocSource::new(&edited, &fields),
                &Edit::Block(path.clone()),
            );
            times.push(started.elapsed().as_secs_f64() * 1e6);
        }
        let s = engine.last_stats();
        println!(
            "one-char edit in table cell at {label} ({:?}): median {:.0} us; pages rebuilt {} reused {}",
            path.steps(),
            median(times),
            s.pages_rebuilt,
            s.pages_reused
        );
    }
    let mut packet = engine.layout(&[DocSource::new(doc, &Values(false))]);
    let started = Instant::now();
    let changed = engine.refresh_fields(&mut packet, &[DocSource::new(doc, &Values(true))]);
    println!(
        "field refresh labels -> values: {changed} blocks re-laid, {:.1} ms, {} pages",
        started.elapsed().as_secs_f64() * 1e3,
        packet.page_count()
    );
    let started = Instant::now();
    let unchanged = engine.refresh_fields(&mut packet, &[DocSource::new(doc, &Values(true))]);
    println!(
        "field refresh with no change: {unchanged} blocks, {:.0} us",
        started.elapsed().as_secs_f64() * 1e6
    );
    let packet = engine.layout(&[DocSource::new(doc, &fields)]);
    let mut pdf_times = Vec::new();
    let mut size = 0;
    for _ in 0..3 {
        let started = Instant::now();
        let (bytes, _) = write_packet(
            &packet,
            &PdfOptions {
                title: None,
                compress: true,
            },
        );
        pdf_times.push(started.elapsed().as_secs_f64() * 1e3);
        size = bytes.len();
    }
    println!(
        "pdf: median {:.1} ms, {} KB for {} pages",
        median(pdf_times),
        size / 1024,
        packet.page_count()
    );
}
