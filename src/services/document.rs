use crate::domain::error::AppError;
use crate::{domain::pixels::Pixels, services::ports::PdfEngine};
use std::{
    path::Path,
    sync::{atomic::AtomicU64, Arc},
};

pub fn raster(
    engine: &impl PdfEngine,
    key: &crate::domain::document::RenderKey,
    cache: &Path,
    generation: &Arc<AtomicU64>,
    expected: u64,
) -> Result<Pixels, AppError> {
    let mut image = engine.raster(
        key.document.path(),
        key.page,
        key.edge,
        cache,
        generation,
        expected,
    )?;
    if key.dark {
        for pixel in image.pixels_mut() {
            pixel[0] = 255 - pixel[0];
            pixel[1] = 255 - pixel[1];
            pixel[2] = 255 - pixel[2];
        }
    }
    Ok(Pixels {
        size: [image.width() as usize, image.height() as usize],
        rgba: image.pixels().map(|p| p.0).collect(),
    })
}
