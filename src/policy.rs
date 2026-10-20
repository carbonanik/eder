//! Central resource policies, measured/tuned independently from module structure.
pub const PAGE_CACHE_BUDGET: usize = 160 * 1024 * 1024;
pub const THUMBNAIL_ENTRIES: usize = 64;
pub const MAX_DOCUMENT_PAGES: u32 = 20_000;
pub const MAX_LINK_PAGES: usize = 64;
pub const POSITION_SAVE_DELAY: std::time::Duration = std::time::Duration::from_millis(650);

pub const PAGE_EDGE: u32 = 2400;
pub const THUMB_EDGE: u32 = 192;
// Includes reading-page PNGs and thumbnails; eviction occurs off the UI/render workers.
pub const DISK_BUDGET: u64 = 384 * 1024 * 1024;

/// Use coarse resolution steps to avoid rerendering for each tiny resize/zoom change.
pub fn edge_bucket(desired_pixels: f32) -> u32 {
    const BUCKETS: [u32; 8] = [512, 704, 896, 1152, 1408, 1728, 2048, PAGE_EDGE];
    let pixels = if desired_pixels.is_finite() {
        desired_pixels.max(1.0)
    } else {
        1152.0
    };
    BUCKETS
        .into_iter()
        .find(|&edge| edge as f32 >= pixels)
        .unwrap_or(PAGE_EDGE)
}
