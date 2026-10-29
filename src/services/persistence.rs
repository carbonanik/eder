use super::ports::LibraryRepository;
use crate::domain::reading_position::ReadingPosition;
use std::path::Path;
pub struct PersistenceService<R> {
    repository: R,
}
impl<R: LibraryRepository> PersistenceService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
    pub fn annotations(
        &self,
        path: &Path,
    ) -> Result<Vec<crate::domain::annotation::Annotation>, String> {
        self.repository.annotations(path)
    }
    pub fn save_annotation(
        &mut self,
        path: &Path,
        item: &crate::domain::annotation::Annotation,
    ) -> Result<(), String> {
        item.validate()?;
        self.repository.save_annotation(path, item)
    }
    pub fn delete_annotation(&self, path: &Path, id: i64) -> Result<(), String> {
        self.repository.delete_annotation(path, id)
    }
    pub fn snapshot(&self) -> Result<crate::domain::settings::Snapshot, String> {
        self.repository.snapshot()
    }
    pub fn take_pending(&mut self) -> Result<Option<std::path::PathBuf>, String> {
        self.repository.take_pending()
    }
    pub fn root(&self, path: &Path) -> Result<(), String> {
        self.repository.root(path)
    }
    pub fn favorite(&self, path: &Path, value: bool) -> Result<(), String> {
        self.repository.favorite(path, value)
    }
    pub fn remove_history(&self, path: &Path) -> Result<(), String> {
        self.repository.remove_history(path)
    }
    pub fn reading_position(&self, path: &Path) -> Result<ReadingPosition, String> {
        self.repository.reading_position(path)
    }
    pub fn opened(&mut self, path: &Path, recovery: bool) -> Result<(), String> {
        self.repository.opened(path, recovery)
    }
    pub fn save_reading_position(
        &self,
        path: &Path,
        position: ReadingPosition,
    ) -> Result<(), String> {
        self.repository.save_reading_position(path, position)
    }
    pub fn dark_theme(&self, value: bool) -> Result<(), String> {
        self.repository.dark_theme(value)
    }
    pub fn dark(&self, value: bool) -> Result<(), String> {
        self.repository.dark(value)
    }
    pub fn continuous_scroll(&self, value: bool) -> Result<(), String> {
        self.repository.continuous_scroll(value)
    }
    pub fn clear_pending(&self) -> Result<(), String> {
        self.repository.clear_pending()
    }
    pub fn finish_session(
        &mut self,
        position: Option<(&Path, ReadingPosition)>,
    ) -> Result<(), String> {
        self.repository.finish_session(position)
    }
}
