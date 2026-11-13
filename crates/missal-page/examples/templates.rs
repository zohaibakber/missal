use std::collections::HashMap;
use std::io::Read;
use std::time::Instant;

use flate2::read::GzDecoder;
use missal_core::document::{Block, Child, Envelope, Inline, ListItemChild};
use missal_core::{Document, FieldDisplayText, FieldReference};
use missal_page::pdf::{PdfOptions, write_packet};
use missal_page::{DocSource, FieldSource, PageEngine, TokenFields};
use serde_json::{Value, json};

const BUNDLE: &[u8] = include_bytes!("../../../templates/bundle.json.gz");

fn bundle() -> Vec<(String, Document)> {
    let mut text = String::new();
    GzDecoder::new(BUNDLE).read_to_string(&mut text).unwrap();
    let Value::Array(items) = serde_json::from_str(&text).unwrap() else {
        panic!("bundle is not an array");
    };
    items
        .into_iter()
        .map(|item| {
            let name = item["name"].as_str().unwrap().to_owned();
            let envelope = Envelope::from_value(item["document"].clone()).unwrap();
            (name, Document::from_envelope(&envelope).unwrap())
        })
        .collect()
}

struct Values(HashMap<String, String>);

impl FieldSource for Values {
    fn display(&self, reference: &FieldReference) -> FieldDisplayText {
        let base = TokenFields.display(reference);
        match self.0.get(&base.text) {
            Some(v) if !v.trim().is_empty() => FieldDisplayText {
                text: v.clone(),
                unresolved: false,
            },
            _ => base,
        }
    }
}

fn inline_fields(inlines: &[Inline], fields: &dyn FieldSource, out: &mut Vec<Value>) {
    for inline in inlines {
        match inline {
            Inline::Field(f) => {
                let d = fields.display(&f.reference);
                out.push(json!({"text": d.text, "unresolved": d.unresolved}));
            }
            Inline::Link(l) => inline_fields(&l.children, fields, out),
            _ => {}
        }
    }
}

fn block_fields(blocks: &[Block], fields: &dyn FieldSource, out: &mut Vec<Value>) {
    for block in blocks {
        match block {
            Block::Paragraph(p) => inline_fields(&p.children, fields, out),
            Block::Heading(h) => inline_fields(&h.children, fields, out),
            Block::Quote(q) => inline_fields(&q.children, fields, out),
            Block::Table(t) => {
                for row in t.rows.iter().filter_map(Child::node) {
                    for cell in row.cells.iter().filter_map(Child::node) {
                        block_fields(&cell.blocks, fields, out);
                    }
                }
            }
            Block::List(l) => {
                for item in l.items.iter().filter_map(Child::node) {
                    for child in &item.children {
                        match child {
                            ListItemChild::Inline(i) => inline_fields(std::slice::from_ref(i), fields, out),
                            ListItemChild::List(n) => block_fields(&[Block::List(n.clone())], fields, out),
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn path_string(path: &missal_page::BlockPath) -> String {
    path.steps().iter().map(|s| s.to_string()).collect::<Vec<_>>().join(".")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let opt = |name: &str| args.iter().position(|a| a == name).map(|i| args[i + 1].clone());
    let out = opt("--out").unwrap_or_else(|| ".".into());
    let filter: Vec<String> = opt("--templates")
        .map(|t| t.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    let values: Option<Values> = opt("--values").map(|path| {
        let v: HashMap<String, String> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        Values(v)
    });
    let fields: &dyn FieldSource = match &values {
        Some(v) => v,
        None => &TokenFields,
    };
    let tag = opt("--tag").unwrap_or_else(|| "missal".into());
    let all = bundle();
    let mut engine = PageEngine::new();
    for (name, document) in &all {
        if !filter.is_empty() && !filter.iter().any(|f| name == f || name.starts_with(f.as_str())) {
            continue;
        }
        let slug: String = name.as_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>().chars().take(24).collect();
        let dir = format!("{out}/{slug}");
        std::fs::create_dir_all(&dir).unwrap();
        let started = Instant::now();
        let packet = engine.layout(&[DocSource::new(document, fields)]);
        let elapsed = started.elapsed().as_secs_f64() * 1e3;
        let lines: Vec<Value> = packet
            .lines(0)
            .iter()
            .map(|l| {
                json!({"path": path_string(&l.path), "line": l.line, "page": l.page, "start": l.text.start,
                "rect": [l.rect.x, l.rect.y, l.rect.width, l.rect.height]})
            })
            .collect();
        let cells: Vec<Value> = packet
            .cells(0)
            .iter()
            .map(|c| json!({"path": path_string(&c.path), "page": c.page, "rect": [c.rect.x, c.rect.y, c.rect.width, c.rect.height]}))
            .collect();
        let mut field_list = Vec::new();
        block_fields(&document.blocks, fields, &mut field_list);
        std::fs::write(format!("{dir}/fields.json"), serde_json::to_string(&field_list).unwrap()).unwrap();
        let summary = json!({"name": name, "pages": packet.page_count(), "layout_ms": elapsed, "lines": lines, "cells": cells});
        std::fs::write(format!("{dir}/{tag}.json"), serde_json::to_string(&summary).unwrap()).unwrap();
        let pdf_started = Instant::now();
        let (bytes, stats) = write_packet(&packet, &PdfOptions { title: Some(name.clone()), compress: true });
        let pdf_ms = pdf_started.elapsed().as_secs_f64() * 1e3;
        std::fs::write(format!("{dir}/{tag}.pdf"), &bytes).unwrap();
        println!(
            "{name}: {} pages, layout {elapsed:.1} ms, pdf {pdf_ms:.1} ms, {} KB, fonts {:?} -> {dir}",
            packet.page_count(),
            bytes.len() / 1024,
            stats.fonts
        );
    }
}
