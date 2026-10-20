use std::{fs, path::Path};
pub fn trim_cache(cache: &Path, budget: u64) -> std::io::Result<()> {
    let mut files: Vec<_> = fs::read_dir(cache)?
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|e| e == "png"))
        .filter_map(|e| {
            e.metadata()
                .ok()
                .map(|m| (e.path(), m.len(), m.modified().ok()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort_by_key(|f| f.2);
    for (p, size, _) in files {
        if total <= budget {
            break;
        }
        if fs::remove_file(p).is_ok() {
            total = total.saturating_sub(size);
        }
    }
    Ok(())
}
