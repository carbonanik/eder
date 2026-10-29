use eder::render;
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
    // A valid PNG with the wrong extent must not evade the thumbnail budget.
    let oversized = image::RgbaImage::from_pixel(256, 256, image::Rgba([255, 0, 0, 255]));
    for file in std::fs::read_dir(cache.path()).unwrap() {
        oversized.save(file.unwrap().path()).unwrap();
    }
    let regenerated = render::raster(&path, 1, render::THUMB_EDGE, cache.path(), &gen, 1).unwrap();
    assert_eq!(
        regenerated.width().max(regenerated.height()),
        render::THUMB_EDGE
    );
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

/// Small deterministic PDF with portrait, landscape, rotated and cropped pages.
fn mixed_geometry_pdf(path: &std::path::Path) {
    let content = "BT /F1 18 Tf 50 550 Td (Go to landscape) Tj ET\n";
    let text_stream = format!(
        "<< /Length {} >>\nstream\n{content}endstream",
        content.len()
    );
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R 6 0 R] /Count 4 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 600] /Resources << >> /Contents 7 0 R >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 800 400] /Resources << >> /Contents 7 0 R >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 600] /Rotate 90 /Resources << /Font << /F1 9 0 R >> >> /Contents 8 0 R /Annots [10 0 R] >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 600] /CropBox [0 0 200 200] /Resources << /Font << /F1 9 0 R >> >> /Contents 8 0 R /Annots [10 0 R] >>",
        "<< /Length 0 >>\nstream\nendstream",
        text_stream.as_str(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        "<< /Type /Annot /Subtype /Link /Rect [45 540 210 575] /Border [0 0 0] /Dest [4 0 R /Fit] >>",
    ];
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
    }
    let xref = pdf.len();
    pdf.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objects.len() + 1
    ));
    for offset in offsets {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    ));
    std::fs::write(path, pdf).unwrap();
}

#[test]
#[ignore = "Requires Poppler executables on PATH"]
fn page_geometry_matches_rotated_and_media_box_rasters() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mixed.pdf");
    mixed_geometry_pdf(&path);
    let generation = Arc::new(AtomicU64::new(1));
    let geometry = render::page_geometry(&path, 4, &generation, 1).unwrap();
    assert_eq!((geometry[0].width, geometry[0].height), (300.0, 600.0));
    assert_eq!((geometry[1].width, geometry[1].height), (800.0, 400.0));
    assert_eq!((geometry[2].width, geometry[2].height), (600.0, 300.0));
    assert_eq!((geometry[3].width, geometry[3].height), (300.0, 600.0));
    // Poppler emits zero-width text bounds on this rotated link; invalid hitboxes
    // remain noninteractive. Rotation itself is checked against the actual raster.
    assert!(render::page_links(&path, 3, 4, &generation, 1)
        .unwrap()
        .is_empty());
    let links = render::page_links(&path, 4, 4, &generation, 1).unwrap();
    assert!(
        !links.is_empty(),
        "Media-box link text must survive crop boxes"
    );
    assert!(links.iter().all(|link| link.target_page == 2));
    for (index, dimensions) in geometry.iter().enumerate() {
        let image =
            render::raster(&path, index as u32 + 1, 512, dir.path(), &generation, 1).unwrap();
        let expected = dimensions.width / dimensions.height;
        assert!((image.width() as f32 / image.height() as f32 - expected).abs() < 0.02);
    }
}

#[test]
#[ignore = "Requires Poppler executables on PATH"]
fn selectable_text_matches_rotated_and_cropped_rasters() {
    use eder::{infrastructure::pdf::poppler::Poppler, services::ports::PdfEngine};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mixed.pdf");
    mixed_geometry_pdf(&path);
    let generation = Arc::new(AtomicU64::new(1));
    let geometry = Poppler.geometry(&path, 4, &generation, 1).unwrap();
    for page in [3, 4] {
        let words = Poppler
            .text(&path, page, geometry[page as usize - 1], &generation, 1)
            .unwrap();
        assert_eq!(
            words
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            "Go to landscape"
        );
        let image = render::raster(&path, page, 1200, dir.path(), &generation, 1).unwrap();
        for word in words {
            // Every extracted word box must cover real ink in the displayed raster.
            let [l, t, r, b] = word.bounds;
            let mut ink = 0;
            for y in (t * image.height() as f32) as u32..(b * image.height() as f32).ceil() as u32 {
                for x in (l * image.width() as f32) as u32..(r * image.width() as f32).ceil() as u32
                {
                    if image.get_pixel(x.min(image.width() - 1), y.min(image.height() - 1))[0] < 128
                    {
                        ink += 1;
                    }
                }
            }
            assert!(
                ink > 5,
                "Word box must align with raster ink on page {page}"
            );
        }
    }
}
