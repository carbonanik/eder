use std::{
    fs,
    path::{Path, PathBuf},
};

pub use crate::domain::document::Entry;
impl Entry {
    pub fn file(path: PathBuf) -> Self {
        let available = path.is_file();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        Self {
            path,
            name,
            directory: false,
            available,
        }
    }
}

pub fn within(root: &Path, requested: &Path) -> Result<PathBuf, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("Library unavailable: {e}. Choose another folder in Settings."))?;
    let path = requested.canonicalize().map_err(|e| e.to_string())?;
    if !path.starts_with(&root) || !path.is_dir() {
        return Err("Folder is outside the configured library.".into());
    }
    Ok(path)
}
pub fn scan(root: &Path, requested: &Path) -> Result<Vec<Entry>, String> {
    let path = within(root, requested)?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut entries = Vec::new();
    for item in fs::read_dir(path).map_err(|e| format!("Cannot read folder: {e}"))? {
        let item = item.map_err(|e| e.to_string())?;
        let path = item.path();
        let ft = item.file_type().map_err(|e| e.to_string())?;
        // Do not allow symlink navigation to escape the library. Broken links are omitted.
        if ft.is_symlink() && !path.canonicalize().is_ok_and(|p| p.starts_with(&root)) {
            continue;
        }
        let directory = path.is_dir();
        let pdf = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
        if directory || (pdf && path.is_file()) {
            entries.push(Entry {
                name: item.file_name().to_string_lossy().into_owned(),
                path: if directory {
                    path
                } else {
                    path.canonicalize().map_err(|e| e.to_string())?
                },
                directory,
                available: true,
            });
        }
    }
    entries.sort_by_cached_key(|e| (!e.directory, e.name.to_lowercase(), e.name.clone()));
    Ok(entries)
}

pub struct LocalFilesystem;
impl crate::services::ports::LibraryFilesystem for LocalFilesystem {
    fn scan(&self, root: &Path, folder: &Path) -> Result<Vec<Entry>, String> {
        scan(root, folder)
    }
    fn within(&self, root: &Path, folder: &Path) -> Result<PathBuf, String> {
        within(root, folder)
    }
    fn import(
        &self,
        sources: Vec<PathBuf>,
        destination: &Path,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, String> {
        let mut imported = 0_usize;
        for source in sources {
            if cancelled() {
                return Err("Import cancelled.".into());
            }
            if !source.is_file()
                || !source
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
            {
                continue;
            }
            let Some(name) = source.file_name() else {
                continue;
            };
            let stem = source.file_stem().unwrap_or_default().to_string_lossy();
            // create_new prevents replacing any existing document.
            for index in 0..10000 {
                let filename = if index == 0 {
                    name.to_os_string()
                } else {
                    format!("{stem} ({index}).pdf").into()
                };
                let target = destination.join(filename);
                match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target)
                {
                    Ok(mut output) => {
                        let result = (|| -> std::io::Result<()> {
                            let mut input = std::fs::File::open(&source)?;
                            let mut buffer = [0_u8; 64 * 1024];
                            loop {
                                if cancelled() {
                                    return Err(std::io::Error::new(
                                        std::io::ErrorKind::Interrupted,
                                        "Import cancelled",
                                    ));
                                }
                                let count = std::io::Read::read(&mut input, &mut buffer)?;
                                if count == 0 {
                                    break;
                                }
                                std::io::Write::write_all(&mut output, &buffer[..count])?;
                            }
                            output.sync_all()
                        })();
                        if let Err(error) = result {
                            drop(output);
                            let _ = std::fs::remove_file(&target);
                            return Err(format!("Could not import {}: {error}", source.display()));
                        }
                        imported += 1;
                        break;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => return Err(format!("Could not create PDF: {error}")),
                }
            }
        }

        Ok(imported)
    }
}

pub fn revision(
    path: &Path,
) -> Result<crate::domain::document::FileRevision, crate::domain::error::AppError> {
    let metadata = fs::metadata(path)?;
    Ok(crate::domain::document::FileRevision {
        bytes: metadata.len(),
        modified_nanos: metadata
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immediate_case_insensitive_sorted_and_confined() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("zFolder")).unwrap();
        for name in ["z.PDF", "A.pdf", "ignore.txt"] {
            fs::write(d.path().join(name), b"test").unwrap();
        }
        fs::write(d.path().join("zFolder/hidden.pdf"), b"test").unwrap();
        let rows = scan(d.path(), d.path()).unwrap();
        assert_eq!(
            rows.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["zFolder", "A.pdf", "z.PDF"]
        );
        assert!(within(d.path(), d.path().parent().unwrap()).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_omitted() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
        assert!(scan(root.path(), root.path()).unwrap().is_empty());
        assert!(within(root.path(), &root.path().join("escape")).is_err());
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;
    use crate::services::ports::LibraryFilesystem;
    #[test]
    fn imports_preserve_existing_sources_and_cancel_without_partial_files() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let path = source.path().join("book.pdf");
        fs::write(&path, b"new content").unwrap();
        fs::write(destination.path().join("book.pdf"), b"existing").unwrap();
        assert_eq!(
            LocalFilesystem
                .import(vec![path.clone()], destination.path(), &|| false)
                .unwrap(),
            1
        );
        assert_eq!(
            fs::read(destination.path().join("book.pdf")).unwrap(),
            b"existing"
        );
        assert_eq!(
            fs::read(destination.path().join("book (1).pdf")).unwrap(),
            b"new content"
        );
        assert_eq!(fs::read(&path).unwrap(), b"new content");
        let checks = std::cell::Cell::new(0);
        assert!(LocalFilesystem
            .import(vec![path], destination.path(), &|| {
                let next = checks.get() + 1;
                checks.set(next);
                next > 1
            })
            .is_err());
        assert!(!destination.path().join("book (2).pdf").exists());
    }
}
