//! Poppler runs in a cancellable child process: no PDF parsing on the UI thread.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant, UNIX_EPOCH},
};

pub const PAGE_EDGE: u32 = 2400;
pub const THUMB_EDGE: u32 = 192;
pub const DISK_BUDGET: u64 = 128 * 1024 * 1024;

fn program(name: &str) -> PathBuf {
    // Optional explicit, trusted Poppler installation directory; never searched in the library.
    std::env::var_os("FOLIO_POPPLER_DIR")
        .map(PathBuf::from)
        .map(|p| {
            p.join(if cfg!(windows) {
                format!("{name}.exe")
            } else {
                name.into()
            })
        })
        .unwrap_or_else(|| name.into())
}
fn execute(
    mut command: Command,
    generation: &Arc<AtomicU64>,
    expected: u64,
) -> Result<Vec<u8>, String> {
    let mut out = tempfile::tempfile().map_err(|e| e.to_string())?;
    let mut err = tempfile::tempfile().map_err(|e| e.to_string())?;
    command
        .stdin(Stdio::null())
        .stdout(out.try_clone().map_err(|e| e.to_string())?)
        .stderr(err.try_clone().map_err(|e| e.to_string())?);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|e| {
        format!("Cannot start Poppler: {e}. Install pdfinfo and pdftoppm; see README.")
    })?;
    let start = Instant::now();
    loop {
        if generation.load(Ordering::Acquire) != expected
            || start.elapsed() > Duration::from_secs(30)
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Rendering cancelled or exceeded 30 seconds.".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    let _ = err.seek(SeekFrom::Start(0));
                    let mut message = String::new();
                    let _ = err.take(4096).read_to_string(&mut message);
                    return Err(format!("Cannot open/render PDF: {}", message.trim()));
                }
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
    }
    out.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    out.take(65536)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}
pub fn page_count(path: &Path, generation: &Arc<AtomicU64>, expected: u64) -> Result<u32, String> {
    if !path.is_file() {
        return Err("This PDF is no longer available.".into());
    }
    let mut command = Command::new(program("pdfinfo"));
    command.env("LC_ALL", "C").arg(path);
    let bytes = execute(command, generation, expected)?;
    String::from_utf8_lossy(&bytes)
        .lines()
        .find_map(|l| {
            l.strip_prefix("Pages:")
                .and_then(|n| n.trim().parse::<u32>().ok())
        })
        .filter(|n| *n > 0)
        .ok_or_else(|| "The document has no readable pages.".into())
}
fn key(path: &Path) -> Result<String, String> {
    let m = fs::metadata(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    hash.update(path.as_os_str().as_encoded_bytes());
    hash.update(m.len().to_le_bytes());
    hash.update(
        m.modified()
            .map_err(|e| e.to_string())?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_le_bytes(),
    );
    Ok(format!("{:x}", hash.finalize()))
}
fn decode(path: &Path) -> Result<image::RgbaImage, String> {
    let mut reader = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(PAGE_EDGE);
    limits.max_image_height = Some(PAGE_EDGE);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map(|i| i.to_rgba8())
        .map_err(|e| e.to_string())
}
pub fn raster(
    path: &Path,
    page: u32,
    edge: u32,
    cache: &Path,
    generation: &Arc<AtomicU64>,
    expected: u64,
) -> Result<image::RgbaImage, String> {
    if !(1..=PAGE_EDGE).contains(&edge) || page == 0 {
        return Err("Invalid render dimensions or page.".into());
    }
    let thumb = edge == THUMB_EDGE && page == 1;
    let cached = key(path).ok().map(|key| cache.join(format!("{key}.png")));
    if thumb {
        if let Some(p) = &cached {
            if let Ok(image) = decode(p) {
                return Ok(image);
            }
        }
    }
    // Cache failure must never prevent viewing a document.
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let prefix = temp.path().join("page");
    let mut command = Command::new(program("pdftoppm"));
    command
        .args([
            "-f",
            &page.to_string(),
            "-l",
            &page.to_string(),
            "-singlefile",
            "-scale-to",
            &edge.to_string(),
            "-png",
        ])
        .arg(path)
        .arg(&prefix);
    execute(command, generation, expected)?;
    let output = prefix.with_extension("png");
    let result = decode(&output)?;
    if thumb {
        if let Some(p) = cached {
            if fs::create_dir_all(cache).is_ok() {
                let _ = fs::copy(&output, p);
                let _ = trim_cache(cache, DISK_BUDGET);
            }
        }
    }
    Ok(result)
}
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
