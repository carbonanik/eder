use crate::domain::{document::PageGeometry, reading_position::ReadingPosition};
use std::ops::Range;

/// Cached geometry is independent of egui. Rebuild only on geometry/width/zoom changes.
#[derive(Clone, Debug)]
pub struct PageLayout {
    pub width: f32,
    pub zoom: f32,
    pub gap: f32,
    pub starts: Vec<f32>,
    pub heights: Vec<f32>,
    pub total_height: f32,
}
impl PageLayout {
    pub fn new(geometry: &[PageGeometry], width: f32, zoom: f32, gap: f32) -> Self {
        let width = width.max(1.0);
        let zoom = zoom.clamp(0.5, 3.0);
        let heights: Vec<_> = geometry
            .iter()
            .map(|g| width * zoom * g.height / g.width)
            .collect();
        let mut y = 0.0;
        let mut starts = Vec::with_capacity(heights.len());
        for &height in &heights {
            starts.push(y);
            y += height + gap;
        }
        let total_height = (y - gap).max(0.0);
        Self {
            width,
            zoom,
            gap,
            starts,
            heights,
            total_height,
        }
    }
    pub fn visible(&self, min: f32, max: f32) -> Range<usize> {
        if self.starts.is_empty() {
            return 0..0;
        }
        let first = self
            .starts
            .partition_point(|start| *start <= min)
            .saturating_sub(1);
        let last = self
            .starts
            .partition_point(|start| *start < max)
            .max(first + 1)
            .min(self.starts.len());
        first..last
    }
    pub fn position(&self, offset: f32) -> ReadingPosition {
        if self.starts.is_empty() {
            return ReadingPosition::new(1, 0.0);
        }
        let offset = if offset.is_finite() {
            offset.max(0.0)
        } else {
            0.0
        };
        let mut index = self
            .starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1);
        let mut fraction = (offset - self.starts[index]) / self.heights[index];
        if fraction >= 1.0 && index + 1 < self.starts.len() {
            index += 1;
            fraction = 0.0;
        }
        ReadingPosition::content(index as u32 + 1, fraction, self.zoom, 0.0)
    }
    /// Reconstruct old fixed-row image placement before switching anchor semantics.
    pub fn convert_legacy(
        &self,
        anchor: ReadingPosition,
        old_viewport_width: f32,
        old_row_gap: f32,
        old_label_height: f32,
    ) -> ReadingPosition {
        let index = anchor.page.saturating_sub(1) as usize;
        if anchor.anchor_version != 0 || index >= self.heights.len() {
            return anchor;
        }
        if anchor.fraction == 0.0 {
            return ReadingPosition::content(anchor.page, 0.0, self.zoom, 0.0);
        }
        let old_zoom = anchor.zoom;
        let row_height = (old_viewport_width * 1.42 * old_zoom).max(300.0);
        let aspect = self.heights[index] / (self.width * self.zoom);
        let image_height = (((old_viewport_width - 24.0).max(1.0) * old_zoom) * aspect)
            .min((row_height - 42.0).max(1.0));
        let fraction =
            (anchor.fraction * (row_height + old_row_gap) - old_label_height - old_row_gap)
                / image_height;
        if fraction >= 1.0 && index + 1 < self.heights.len() {
            ReadingPosition::content(anchor.page + 1, 0.0, self.zoom, 0.0)
        } else {
            ReadingPosition::content(anchor.page, fraction, self.zoom, 0.0)
        }
    }
    pub fn offset(&self, position: ReadingPosition) -> f32 {
        let index = position.page.saturating_sub(1) as usize;
        if index >= self.starts.len() {
            return self.total_height;
        }
        self.starts[index] + position.fraction * self.heights[index]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_fraction_is_converted_from_old_row_geometry() {
        let geometry = [PageGeometry::new(800.0, 400.0, 0).unwrap(); 2];
        let layout = PageLayout::new(&geometry, 800.0, 1.0, 28.0);
        let anchor = ReadingPosition::new(1, 0.2);
        let converted = layout.convert_legacy(anchor, 824.0, 12.0, 18.0);
        let expected = (0.2 * (824.0 * 1.42 + 12.0) - 30.0) / 400.0;
        assert_eq!(converted.anchor_version, 1);
        assert!((converted.fraction - expected).abs() < 0.001);
        assert_eq!(
            layout
                .convert_legacy(ReadingPosition::new(1, 0.9), 824.0, 12.0, 18.0)
                .page,
            2
        );
    }
    #[test]
    fn variable_pages_virtualize_and_preserve_anchor_after_resize() {
        let sizes = [
            PageGeometry::new(400.0, 800.0, 0).unwrap(),
            PageGeometry::new(800.0, 400.0, 0).unwrap(),
        ];
        let layout = PageLayout::new(&sizes, 400.0, 1.0, 20.0);
        assert_eq!(layout.visible(850.0, 900.0), 1..2);
        let anchor = layout.position(900.0);
        assert_eq!(anchor.page, 2);
        assert!((anchor.fraction - 0.4).abs() < 0.001);
        let resized = PageLayout::new(&sizes, 800.0, 1.0, 20.0);
        assert!((resized.offset(anchor) - 1780.0).abs() < 0.001);
        assert_eq!(layout.position(810.0).page, 2);
    }
}
