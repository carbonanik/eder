use folio_reader::render;
use std::{
    path::PathBuf,
    sync::{atomic::AtomicU64, Arc},
};
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.pdf")
}
#[test]
#[ignore = "Requires Poppler executables on PATH"]
fn renders_pages_and_reuses_preview_without_renderer() {
    let gen = Arc::new(AtomicU64::new(1));
    let cache = tempfile::tempdir().unwrap();
    let path = fixture();
    assert_eq!(render::page_count(&path, &gen, 1).unwrap(), 3);
    let thumb = render::raster(&path, 1, render::THUMB_EDGE, cache.path(), &gen, 1).unwrap();
    assert_eq!(thumb.height(), render::THUMB_EDGE);
    // An obsolete generation would cancel a subprocess. A valid disk cache still succeeds.
    let cached = render::raster(&path, 1, render::THUMB_EDGE, cache.path(), &gen, 0).unwrap();
    assert_eq!(thumb, cached);
    let page = render::raster(&path, 2, render::PAGE_EDGE, cache.path(), &gen, 1).unwrap();
    assert_eq!(page.height(), render::PAGE_EDGE);
    // A corrupt preview is regenerated, not propagated as a fatal cache error.
    for f in std::fs::read_dir(cache.path()).unwrap() {
        std::fs::write(f.unwrap().path(), b"broken").unwrap();
    }
    assert!(render::raster(&path, 1, render::THUMB_EDGE, cache.path(), &gen, 1).is_ok());
}
#[test]
#[ignore = "Requires Poppler executables on PATH"]
fn cancellation_invalid_pdf_and_unwritable_cache() {
    let gen = Arc::new(AtomicU64::new(9));
    let cache = tempfile::tempdir().unwrap();
    assert!(render::page_count(&fixture(), &gen, 8).is_err());
    let bad = cache.path().join("bad.pdf");
    std::fs::write(&bad, b"not a pdf").unwrap();
    assert!(render::page_count(&bad, &gen, 9).is_err());
    assert!(render::raster(&fixture(), 1, render::THUMB_EDGE, &bad, &gen, 9).is_ok());
    assert!(render::raster(&fixture(), 0, render::PAGE_EDGE, cache.path(), &gen, 9).is_err());
}
#[test]
fn disk_cache_has_a_budget() {
    let cache = tempfile::tempdir().unwrap();
    for i in 0..10 {
        std::fs::write(cache.path().join(format!("{i}.png")), [0u8; 100]).unwrap();
    }
    render::trim_cache(cache.path(), 250).unwrap();
    let size: u64 = std::fs::read_dir(cache.path())
        .unwrap()
        .map(|p| p.unwrap().metadata().unwrap().len())
        .sum();
    assert!(size <= 250);
}
