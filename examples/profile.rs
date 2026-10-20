//! Reproducible smoke measurements, not a comprehensive performance benchmark.
use eder::{library, render};
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
    let geometry = render::page_geometry(&fixture, pages, &gen, 0).unwrap();
    println!(
        "PDF page geometry: {:?} ({} pages)",
        start.elapsed(),
        geometry.len()
    );
    let sizes: Vec<_> = (0..20_000)
        .map(|index| geometry[index % geometry.len()])
        .collect();
    let start = Instant::now();
    let layout = eder::app::reader::layout::PageLayout::new(&sizes, 800.0, 1.0, 28.0);
    println!("20,000-page layout preparation: {:?}", start.elapsed());
    let start = Instant::now();
    for index in 0..100_000 {
        let y = (index % 20_000) as f32 * 300.0;
        std::hint::black_box(layout.visible(y, y + 900.0));
    }
    println!("100,000 viewport range lookups: {:?}", start.elapsed());
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
