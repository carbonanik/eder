use crate::domain::document::PdfLink;
use std::{
    path::Path,
    sync::{atomic::AtomicU64, Arc},
};

/// A backend is constructed and retained by its worker, without Send/Sync requirements.
pub trait PdfEngine {
    fn page_count(
        &self,
        path: &Path,
        generation: &Arc<AtomicU64>,
        expected: u64,
    ) -> Result<u32, crate::domain::error::AppError>;
    fn geometry(
        &self,
        path: &Path,
        total: u32,
        generation: &Arc<AtomicU64>,
        expected: u64,
    ) -> Result<Vec<crate::domain::document::PageGeometry>, crate::domain::error::AppError>;
    fn raster(
        &self,
        path: &Path,
        page: u32,
        edge: u32,
        cache: &Path,
        generation: &Arc<AtomicU64>,
        expected: u64,
    ) -> Result<image::RgbaImage, crate::domain::error::AppError>;
    fn links(
        &self,
        path: &Path,
        page: u32,
        total: u32,
        generation: &Arc<AtomicU64>,
        expected: u64,
    ) -> Result<Vec<PdfLink>, crate::domain::error::AppError>;
}

pub trait LibraryFilesystem {
    fn scan(
        &self,
        root: &Path,
        folder: &Path,
    ) -> Result<Vec<crate::domain::document::Entry>, String>;
    fn within(&self, root: &Path, folder: &Path) -> Result<std::path::PathBuf, String>;
    fn import(
        &self,
        sources: Vec<std::path::PathBuf>,
        destination: &Path,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, String>;
}

pub trait LibraryRepository {
    fn snapshot(&self) -> Result<crate::domain::settings::Snapshot, String>;
    fn take_pending(&mut self) -> Result<Option<std::path::PathBuf>, String>;
    fn root(&self, path: &Path) -> Result<(), String>;
    fn favorite(&self, path: &Path, value: bool) -> Result<(), String>;
    fn remove_history(&self, path: &Path) -> Result<(), String>;
    fn reading_position(
        &self,
        path: &Path,
    ) -> Result<crate::domain::reading_position::ReadingPosition, String>;
    fn opened(&mut self, path: &Path, recovery: bool) -> Result<(), String>;
    fn save_reading_position(
        &self,
        path: &Path,
        position: crate::domain::reading_position::ReadingPosition,
    ) -> Result<(), String>;
    fn dark(&self, value: bool) -> Result<(), String>;
    fn dark_theme(&self, value: bool) -> Result<(), String>;
    fn continuous_scroll(&self, value: bool) -> Result<(), String>;
    fn clear_pending(&self) -> Result<(), String>;
    fn finish_session(
        &mut self,
        position: Option<(&Path, crate::domain::reading_position::ReadingPosition)>,
    ) -> Result<(), String>;
}
