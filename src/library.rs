use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub directory: bool,
    pub available: bool,
}
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
