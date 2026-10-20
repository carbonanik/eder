use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub directory: bool,
    pub available: bool,
}

/// A clickable internal destination within one rendered PDF page.
/// Bounds are normalized to the page's displayed image (top-left origin).
#[derive(Clone, Debug, PartialEq)]
pub struct PdfLink {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub target_page: u32,
}

/// Dimensions are in PDF points after applying the intrinsic page rotation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageGeometry {
    pub width: f32,
    pub height: f32,
}
impl PageGeometry {
    pub fn new(width: f32, height: f32, rotation: i32) -> Option<Self> {
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return None;
        }
        let (width, height) = if rotation.rem_euclid(180) == 90 {
            (height, width)
        } else {
            (width, height)
        };
        Some(Self { width, height })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DocumentId(PathBuf);
impl DocumentId {
    /// Infrastructure supplies a canonical native path after validating the file.
    pub fn from_canonical(path: PathBuf) -> Self {
        Self(path)
    }
    pub fn path(&self) -> &std::path::Path {
        &self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileRevision {
    pub bytes: u64,
    pub modified_nanos: u128,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RenderKey {
    pub document: DocumentId,
    pub revision: FileRevision,
    pub page: u32,
    pub edge: u32,
    pub dark: bool,
    /// Intrinsic rotation, media-box rendering and pixel transform conventions.
    pub backend_version: u32,
}
