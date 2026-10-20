use crate::domain::error::AppError;
use std::{
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
pub(super) fn program(name: &str) -> PathBuf {
    // Optional explicit, trusted Poppler installation directory; never searched in the library.
    std::env::var_os("EDER_POPPLER_DIR")
        .or_else(|| std::env::var_os("FOLIO_POPPLER_DIR"))
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
pub(super) fn execute(
    command: Command,
    generation: &Arc<AtomicU64>,
    expected: u64,
) -> Result<Vec<u8>, AppError> {
    execute_limited(command, generation, expected, 2 * 1024 * 1024)
}
pub(super) fn execute_limited(
    mut command: Command,
    generation: &Arc<AtomicU64>,
    expected: u64,
    max_output: u64,
) -> Result<Vec<u8>, AppError> {
    if generation.load(Ordering::Acquire) != expected {
        return Err(AppError::Cancelled);
    }
    let mut out = tempfile::tempfile().map_err(AppError::from)?;
    let mut err = tempfile::tempfile().map_err(AppError::from)?;
    command
        .stdin(Stdio::null())
        .stdout(out.try_clone().map_err(AppError::from)?)
        .stderr(err.try_clone().map_err(AppError::from)?);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| AppError::MissingBackend(e.to_string()))?;
    let start = Instant::now();
    loop {
        if generation.load(Ordering::Acquire) != expected
            || start.elapsed() > Duration::from_secs(30)
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(if generation.load(Ordering::Acquire) != expected {
                AppError::Cancelled
            } else {
                AppError::Timeout
            });
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    let _ = err.seek(SeekFrom::Start(0));
                    let mut message = String::new();
                    let _ = err.take(4096).read_to_string(&mut message);
                    return Err(AppError::InvalidDocument(format!(
                        "Cannot open/render PDF: {}",
                        message.trim()
                    )));
                }
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AppError::Io(e));
            }
        }
    }
    out.seek(SeekFrom::Start(0)).map_err(AppError::from)?;
    let mut bytes = Vec::new();
    // Bounded output: XML with many linked text fragments can exceed pdfinfo's size.
    out.take(max_output + 1)
        .read_to_end(&mut bytes)
        .map_err(AppError::from)?;
    if bytes.len() as u64 > max_output {
        return Err(AppError::OutputLimit);
    }
    Ok(bytes)
}
