use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};

pub struct AppPaths {
    pub data: Option<PathBuf>,
    pub cache: PathBuf,
}
impl AppPaths {
    pub fn discover() -> Self {
        let dirs = directories::ProjectDirs::from("com", "Folio", "Folio");
        Self {
            data: dirs.as_ref().map(|d| d.data_local_dir().to_path_buf()),
            cache: dirs
                .as_ref()
                .map(|d| d.cache_dir().join("previews-v1"))
                .unwrap_or_else(|| std::env::temp_dir().join("folio-previews-v1")),
        }
    }
}
pub fn pick_files(wake: crate::runtime::WakeUi) -> Receiver<Option<Vec<PathBuf>>> {
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(|| {
            rfd::FileDialog::new()
                .add_filter("PDF documents", &["pdf"])
                .set_title("Add PDFs to Eder")
                .pick_files()
        })
        .unwrap_or(None);
        let _ = tx.send(result);
        wake();
    });
    rx
}
pub fn pick_folder(wake: crate::runtime::WakeUi) -> Receiver<Option<PathBuf>> {
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(|| {
            rfd::FileDialog::new()
                .set_title("Choose PDF library")
                .pick_folder()
        })
        .unwrap_or(None);
        let _ = tx.send(result);
        wake();
    });
    rx
}

/// OS lock is released on exit/crash; an existing empty lock file is harmless.
pub struct InstanceLock(std::fs::File);
impl InstanceLock {
    pub fn acquire(path: &std::path::Path) -> Result<Self, String> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|e| format!("Cannot open library lock: {e}"))?;
        file.try_lock().map_err(|e| {
            format!("Cannot lock library data; another Eder instance may be running: {e}")
        })?;
        Ok(Self(file))
    }
}
impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn instance_lock_is_exclusive_and_released_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.lock");
        let first = InstanceLock::acquire(&path).unwrap();
        assert!(InstanceLock::acquire(&path).is_err());
        drop(first);
        assert!(InstanceLock::acquire(&path).is_ok());
    }
}
