use std::time::{Duration, Instant};

use missal_core::{FieldCatalog, FieldMarkers};

fn main() {
    let directory = std::env::args()
        .nth(1)
        .unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/../../templates").to_owned());
    let catalog = FieldCatalog::new(Vec::new());
    let markers = FieldMarkers::default();
    let mut files: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "docx")
        })
        .collect();
    files.sort_by_key(|path| {
        std::fs::metadata(path)
            .map(|metadata| metadata.len())
            .unwrap_or(0)
    });
    let mut total = Duration::ZERO;
    for path in &files {
        let bytes = std::fs::read(path).unwrap();
        let mut samples: Vec<Duration> = (0..15)
            .map(|_| {
                let start = Instant::now();
                let imported = missal_docx::import(&bytes, &catalog, &markers).unwrap();
                let elapsed = start.elapsed();
                std::hint::black_box(imported);
                elapsed
            })
            .collect();
        samples.sort();
        total += samples[samples.len() / 2];
        println!(
            "{:>8} bytes  median {:>7.2} ms  max {:>7.2} ms  {}",
            bytes.len(),
            samples[samples.len() / 2].as_secs_f64() * 1000.0,
            samples[samples.len() - 1].as_secs_f64() * 1000.0,
            path.file_name().unwrap().to_string_lossy()
        );
    }
    println!(
        "all {} files: {:.1} ms (sum of medians)",
        files.len(),
        total.as_secs_f64() * 1000.0
    );
}
