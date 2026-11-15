use std::time::Instant;

use missal_core::{Fir, FirStatus, TemplateId};
use missal_store::{Store, TemplateBundle};

fn main() {
    let firs: usize = std::env::args()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bench.db");
    let bundle = TemplateBundle::embedded();
    let started = Instant::now();
    let documents: Vec<_> = bundle.templates().unwrap();
    println!(
        "bundle decode: {:.1}ms ({} templates)",
        started.elapsed().as_secs_f64() * 1e3,
        documents.len()
    );
    let started = Instant::now();
    for _ in 0..10 {
        for (_, document) in &documents {
            std::hint::black_box(document.encode());
        }
    }
    println!("encode x10: {:.1}ms", started.elapsed().as_secs_f64() * 1e3);
    let encoded: Vec<_> = documents.iter().map(|(_, d)| d.encode()).collect();
    let bytes: usize = encoded.iter().map(|e| e.len()).sum();
    let started = Instant::now();
    for _ in 0..10 {
        for e in &encoded {
            std::hint::black_box(missal_core::Document::decode(e).unwrap());
        }
    }
    println!(
        "decode x10: {:.1}ms, encoded bytes {bytes}",
        started.elapsed().as_secs_f64() * 1e3
    );
    let started = Instant::now();
    let mut store = Store::open(&path).unwrap();
    println!(
        "first open + install: {:.1}ms",
        started.elapsed().as_secs_f64() * 1e3
    );
    let templates: Vec<TemplateId> = store.templates().unwrap().iter().map(|t| t.id).collect();
    let started = Instant::now();
    for index in 0..firs {
        let fir = store
            .create_fir(&Fir {
                fir_no: format!("{index}/2026"),
                date: "01-01-2026".into(),
                offence: "چوری".into(),
                incident_date: "01-01-2026".into(),
                accused: vec!["علی".into()],
                status: FirStatus::Open,
                ..Fir::default()
            })
            .unwrap();
        let pick = [
            templates[index % templates.len()],
            templates[(index * 7 + 3) % templates.len()],
        ];
        store.add_templates_to_fir(fir.id, &pick).unwrap();
    }
    println!(
        "create {firs} firs with 2 documents: {:.1}ms",
        started.elapsed().as_secs_f64() * 1e3
    );
    drop(store);
    let size = std::fs::metadata(&path).unwrap().len();
    println!("database size: {:.1} MiB", size as f64 / 1048576.0);
    let started = Instant::now();
    let store = Store::open(&path).unwrap();
    println!("reopen: {:.1}ms", started.elapsed().as_secs_f64() * 1e3);
    let started = Instant::now();
    let list = store.firs().unwrap();
    println!("list firs: {:.1}ms", started.elapsed().as_secs_f64() * 1e3);
    let started = Instant::now();
    for _ in 0..20 {
        std::hint::black_box(store.templates().unwrap());
    }
    println!(
        "list templates x20: {:.1}ms",
        started.elapsed().as_secs_f64() * 1e3
    );
    let started = Instant::now();
    for fir in list.iter().take(500) {
        std::hint::black_box(store.fir_workspace(fir.id).unwrap());
    }
    println!(
        "500 fir workspaces: {:.1}ms",
        started.elapsed().as_secs_f64() * 1e3
    );
    let started = Instant::now();
    for fir in list.iter().take(200) {
        for summary in store.fir_documents(fir.id).unwrap() {
            std::hint::black_box(store.fir_document(summary.id).unwrap());
        }
    }
    println!(
        "load 400 fir documents: {:.1}ms",
        started.elapsed().as_secs_f64() * 1e3
    );
}
