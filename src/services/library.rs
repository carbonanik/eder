use super::ports::LibraryFilesystem;
use crate::domain::document::Entry;
use std::path::{Path, PathBuf};

pub struct LibraryService<F> {
    filesystem: F,
}
impl<F: LibraryFilesystem> LibraryService<F> {
    pub fn new(filesystem: F) -> Self {
        Self { filesystem }
    }
    pub fn scan(&self, root: &Path, folder: &Path) -> Result<Vec<Entry>, String> {
        self.filesystem.scan(root, folder)
    }
    pub fn import(
        &self,
        sources: Vec<PathBuf>,
        root: &Path,
        folder: &Path,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, String> {
        let destination = self.filesystem.within(root, folder)?;
        self.filesystem.import(sources, &destination, cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct ConfinedFilesystem;
    impl LibraryFilesystem for ConfinedFilesystem {
        fn scan(&self, _root: &Path, _folder: &Path) -> Result<Vec<Entry>, String> {
            Ok(Vec::new())
        }
        fn within(&self, _root: &Path, _folder: &Path) -> Result<PathBuf, String> {
            Err("outside library".into())
        }
        fn import(
            &self,
            _sources: Vec<PathBuf>,
            _destination: &Path,
            _cancelled: &dyn Fn() -> bool,
        ) -> Result<usize, String> {
            panic!("must not write outside the root")
        }
    }
    #[test]
    fn rejected_destination_never_starts_an_import() {
        let service = LibraryService::new(ConfinedFilesystem);
        assert!(service
            .import(
                vec!["source.pdf".into()],
                Path::new("root"),
                Path::new("outside"),
                &|| false
            )
            .is_err());
    }
}
