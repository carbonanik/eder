use super::*;
use crate::{
    infrastructure::pdf::poppler::Poppler,
    services::{document, ports::PdfEngine},
};
use std::sync::{Condvar, Mutex};
#[derive(Default)]
struct GateState {
    active: usize,
    foreground_waiting: usize,
}
#[derive(Default)]
pub(super) struct ProcessGate {
    state: Mutex<GateState>,
    changed: Condvar,
}
struct Permit(Arc<ProcessGate>);
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.state.lock().unwrap().active -= 1;
        self.0.changed.notify_all();
    }
}
#[derive(Clone)]
pub(super) struct Lane {
    pub stop: Arc<AtomicBool>,
    pub wake: WakeUi,
    pub permits: Arc<ProcessGate>,
}
impl Lane {
    fn acquire(&self, foreground: bool) -> Option<Permit> {
        let mut state = self.permits.state.lock().unwrap();
        if foreground {
            state.foreground_waiting += 1;
        }
        loop {
            if self.stop.load(Ordering::Acquire) {
                if foreground {
                    state.foreground_waiting -= 1;
                }
                return None;
            }
            // Background work leaves a process slot available for a newly-visible page.
            if (foreground && state.active < 2)
                || (!foreground && state.active < 1 && state.foreground_waiting == 0)
            {
                if foreground {
                    state.foreground_waiting -= 1;
                }
                state.active += 1;
                return Some(Permit(self.permits.clone()));
            }
            state = self
                .permits
                .changed
                .wait_timeout(state, Duration::from_millis(50))
                .unwrap()
                .0;
        }
    }
}
pub(super) fn pages<P: Send + 'static>(
    slot: Arc<Latest<PageJob>>,
    generation: Arc<AtomicU64>,
    cache: PathBuf,
    results: SyncSender<PageResult<P>>,
    maintenance: SyncSender<()>,
    lane: Lane,
    prepare: Arc<dyn Fn(crate::domain::pixels::Pixels) -> P + Send + Sync>,
) {
    let engine = Poppler;
    let mut count = 0_usize;
    while let Some(job) = slot.take() {
        if generation.load(Ordering::Acquire) != job.id {
            continue;
        }
        let Some(permit) = lane.acquire(true) else {
            break;
        };
        let result = crate::infrastructure::filesystem::revision(job.key.document.path())
            .and_then(|revision| {
                if revision != job.key.revision {
                    return Err(crate::domain::error::AppError::InvalidDocument(
                        "The document changed. Reopen it to refresh pages.".into(),
                    ));
                }
                document::raster(&engine, &job.key, &cache, &generation, job.id).and_then(
                    |pixels| {
                        if crate::infrastructure::filesystem::revision(job.key.document.path())?
                            != job.key.revision
                        {
                            return Err(crate::domain::error::AppError::InvalidDocument(
                                "The document changed while rendering. Reopen it.".into(),
                            ));
                        }
                        Ok(pixels)
                    },
                )
            })
            .map(|pixels| prepare(pixels));
        drop(permit);
        if generation.load(Ordering::Acquire) == job.id
            && !deliver(
                &results,
                PageResult {
                    id: job.id,
                    key: job.key,
                    result,
                },
                &lane.stop,
                &lane.wake,
            )
        {
            break;
        }
        count += 1;
        if count.is_multiple_of(24) {
            let _ = maintenance.try_send(());
        }
    }
}
pub(super) fn links(
    jobs: Receiver<LinkJob>,
    generation: Arc<AtomicU64>,
    results: SyncSender<LinkResult>,
    lane: Lane,
) {
    let engine = Poppler;
    while !lane.stop.load(Ordering::Acquire) {
        let job = match jobs.recv_timeout(Duration::from_millis(50)) {
            Ok(job) => job,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        if generation.load(Ordering::Acquire) != job.id {
            continue;
        }
        let Some(permit) = lane.acquire(false) else {
            break;
        };
        let links = engine
            .links(&job.path, job.page, job.total, &generation, job.id)
            .unwrap_or_default();
        let text = engine.text(&job.path, job.page, job.geometry, &generation, job.id);
        drop(permit);
        if generation.load(Ordering::Acquire) == job.id
            && !deliver(
                &results,
                LinkResult {
                    id: job.id,
                    page: job.page,
                    links,
                    text,
                },
                &lane.stop,
                &lane.wake,
            )
        {
            break;
        }
    }
}
pub(super) fn thumbnails<P: Send + 'static>(
    jobs: Receiver<ThumbJob>,
    generation: Arc<AtomicU64>,
    cache: PathBuf,
    results: SyncSender<ThumbResult<P>>,
    lane: Lane,
    prepare: Arc<dyn Fn(crate::domain::pixels::Pixels) -> P + Send + Sync>,
) {
    let engine = Poppler;
    while !lane.stop.load(Ordering::Acquire) {
        let job = match jobs.recv_timeout(Duration::from_millis(50)) {
            Ok(job) => job,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        if generation.load(Ordering::Acquire) != job.id {
            continue;
        }
        let Some(permit) = lane.acquire(false) else {
            break;
        };
        let image = crate::infrastructure::filesystem::revision(&job.path)
            .and_then(|revision| {
                let key = crate::domain::document::RenderKey {
                    document: crate::domain::document::DocumentId::from_canonical(job.path.clone()),
                    revision,
                    page: job.page.unwrap_or(1),
                    edge: crate::policy::THUMB_EDGE,
                    dark: false,
                    backend_version: 1,
                };
                document::raster(&engine, &key, &cache, &generation, job.id)
            })
            .ok()
            .map(|pixels| prepare(pixels));
        drop(permit);
        if generation.load(Ordering::Acquire) == job.id
            && !deliver(
                &results,
                ThumbResult {
                    page: job.page,
                    path: job.path,
                    id: job.id,
                    image,
                },
                &lane.stop,
                &lane.wake,
            )
        {
            break;
        }
    }
}
pub(super) fn opens(
    slot: Arc<Latest<(PathBuf, bool, u64)>>,
    generation: Arc<AtomicU64>,
    tasks: SyncSender<Task>,
    events: SyncSender<Event>,
    lane: Lane,
) {
    let engine = Poppler;
    while let Some((path, recovery, id)) = slot.take() {
        if generation.load(Ordering::Acquire) != id {
            continue;
        }
        let Some(permit) = lane.acquire(true) else {
            break;
        };
        let result = path
            .canonicalize()
            .map_err(crate::domain::error::AppError::from)
            .and_then(|path| {
                let revision = crate::infrastructure::filesystem::revision(&path)?;
                let total = engine.page_count(&path, &generation, id)?;
                let geometry = engine.geometry(&path, total, &generation, id)?;
                if crate::infrastructure::filesystem::revision(&path)? != revision {
                    return Err(crate::domain::error::AppError::InvalidDocument(
                        "The document changed while opening. Try again.".into(),
                    ));
                }
                Ok((path, total, geometry, revision))
            });
        drop(permit);
        if generation.load(Ordering::Acquire) != id {
            continue;
        }
        match result {
            Ok((path, total, geometry, revision)) => {
                if !deliver(
                    &tasks,
                    Task::Opened(path, recovery, id, total, geometry, revision),
                    &lane.stop,
                    &lane.wake,
                ) {
                    break;
                }
            }
            Err(e) => {
                if !deliver(
                    &events,
                    Event::OpenFailed(id, e.to_string()),
                    &lane.stop,
                    &lane.wake,
                ) {
                    break;
                }
            }
        }
    }
}
