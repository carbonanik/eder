/// A stable position within a virtualized continuous-scrolling PDF.
/// Legacy anchors retain row-stride semantics until converted by reader layout.
/// New anchors store a page-content fraction plus zoom and horizontal pan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReadingPosition {
    pub page: u32,
    pub fraction: f32,
    /// 0 = legacy fixed-row stride; 1 = normalized page-content coordinates.
    pub anchor_version: u32,
    pub zoom: f32,
    pub horizontal: f32,
}
impl ReadingPosition {
    pub fn new(page: u32, fraction: f32) -> Self {
        Self {
            page: page.max(1),
            anchor_version: 0,
            zoom: 1.0,
            horizontal: 0.0,
            fraction: if fraction.is_finite() {
                fraction.clamp(0.0, 0.999_999)
            } else {
                0.0
            },
        }
    }

    pub fn content(page: u32, fraction: f32, zoom: f32, horizontal: f32) -> Self {
        let mut position = Self::new(page, fraction);
        position.anchor_version = 1;
        position.zoom = if zoom.is_finite() {
            zoom.clamp(0.5, 3.0)
        } else {
            1.0
        };
        position.horizontal = if horizontal.is_finite() {
            horizontal.clamp(0.0, 1.0)
        } else {
            0.0
        };
        position
    }
    pub fn validated(self) -> Self {
        if self.anchor_version == 0 {
            Self::new(self.page, self.fraction)
        } else {
            Self::content(self.page, self.fraction, self.zoom, self.horizontal)
        }
    }
    pub fn from_scroll_offset(offset: f32, row_stride: f32, total_pages: u32) -> Self {
        if !offset.is_finite() || !row_stride.is_finite() || row_stride <= 0.0 {
            return Self::new(1, 0.0);
        }
        let page_coordinate = (offset / row_stride).max(0.0);
        let max_index = total_pages.saturating_sub(1) as f32;
        let index = page_coordinate.floor().min(max_index);
        Self::new(index as u32 + 1, page_coordinate - index)
    }

    pub fn scroll_offset(self, row_stride: f32) -> f32 {
        (self.page.saturating_sub(1) as f32 + self.fraction) * row_stride
    }
}
