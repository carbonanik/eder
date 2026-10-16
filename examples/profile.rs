//! Reproducible smoke measurements, not a comprehensive performance benchmark.
use folio_reader::{library, render};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicU64, Arc},
    time::Instant,
};
fn main() {
    let root = tempfile::tempdir().unwrap();
    for i in 0..10_000 {
        std::fs::write(root.path().join(format!("Document {i:05}.PDF")), b"fixture").unwrap();
    }
    let start = Instant::now();
    let rows = library::scan(root.path(), root.path()).unwrap();
    println!(
        "10,000-entry scan + sort: {:?} ({} entries)",
        start.elapsed(),
        rows.len()
    );
    let fixture = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.pdf")
        });
    let cache = tempfile::tempdir().unwrap();
    let gen = Arc::new(AtomicU64::new(0));
    let start = Instant::now();
    let pages = render::page_count(&fixture, &gen, 0).unwrap();
    println!("PDF metadata: {:?} ({} pages)", start.elapsed(), pages);
    let start = Instant::now();
    let _ = render::raster(&fixture, 1, render::THUMB_EDGE, cache.path(), &gen, 0).unwrap();
    println!("Cold thumbnail: {:?}", start.elapsed());
    let start = Instant::now();
    let _ = render::raster(&fixture, 1, render::THUMB_EDGE, cache.path(), &gen, 0).unwrap();
    println!("Cached thumbnail: {:?}", start.elapsed());
    let start = Instant::now();
    let img = render::raster(&fixture, 1, render::PAGE_EDGE, cache.path(), &gen, 0).unwrap();
    println!(
        "Page render: {:?} ({} x {}, {} RGBA bytes)",
        start.elapsed(),
        img.width(),
        img.height(),
        img.len()
    );
}
