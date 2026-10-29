//! Supervised, bounded background execution. No dependency on egui.
mod io_worker;
mod lifecycle;
mod pdf_worker;
pub mod queues;

use crate::{
    app::{effect::Task, event::Event, jobs::*},
    infrastructure::platform::AppPaths,
};
use queues::Latest;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

pub type WakeUi = Arc<dyn Fn() + Send + Sync>;
pub(crate) struct RuntimeClient<P = crate::domain::pixels::Pixels> {
    pub tx: SyncSender<Task>,
    pub rx: Receiver<Event>,
    pub page_rx: Receiver<PageResult<P>>,
    pub link_tx: SyncSender<LinkJob>,
    pub link_rx: Receiver<LinkResult>,
    pub link_gen: Arc<AtomicU64>,
    pub thumb_tx: SyncSender<ThumbJob>,
    pub thumb_rx: Receiver<ThumbResult<P>>,
    pub pages: Arc<Latest<PageJob>>,
    pub page_gen: Arc<AtomicU64>,
    pub thumb_gen: Arc<AtomicU64>,
    pub open_gen: Arc<AtomicU64>,
    opens: Arc<Latest<(PathBuf, bool, u64)>>,
    imports: SyncSender<(Vec<PathBuf>, PathBuf, PathBuf)>,
    supervisor: lifecycle::Supervisor,
    shutdown: Arc<Latest<Option<(PathBuf, crate::domain::reading_position::ReadingPosition)>>>,
    closing: Arc<AtomicBool>,
    import_cancel: Arc<AtomicBool>,
    imports_pending: Arc<AtomicUsize>,
}
impl<P: Send + 'static> RuntimeClient<P> {
    pub fn start(
        paths: AppPaths,
        wake: WakeUi,
        prepare: Arc<dyn Fn(crate::domain::pixels::Pixels) -> P + Send + Sync>,
    ) -> Self {
        let (tx, tasks) = mpsc::sync_channel(64);
        let (events, rx) = mpsc::sync_channel(64);
        let (page_results, page_rx) = mpsc::sync_channel(1);
        let (link_tx, links) = mpsc::sync_channel(12);
        let (link_results, link_rx) = mpsc::sync_channel(8);
        let (thumb_tx, thumbs) = mpsc::sync_channel(12);
        let (thumb_results, thumb_rx) = mpsc::sync_channel(4);
        let (imports, import_jobs) = mpsc::sync_channel::<(Vec<PathBuf>, PathBuf, PathBuf)>(1);
        let pages = Arc::new(Latest::default());
        let opens = Arc::new(Latest::default());
        let page_gen = Arc::new(AtomicU64::new(0));
        let link_gen = Arc::new(AtomicU64::new(0));
        let thumb_gen = Arc::new(AtomicU64::new(0));
        let open_gen = Arc::new(AtomicU64::new(0));
        let shutdown = Arc::new(Latest::default());
        let closing = Arc::new(AtomicBool::new(false));
        let import_cancel = Arc::new(AtomicBool::new(false));
        let imports_pending = Arc::new(AtomicUsize::new(0));
        let mut supervisor = lifecycle::Supervisor::new(events.clone(), wake.clone());
        let stop = supervisor.stop.clone();
        let data = paths.data;
        let event_tx = events.clone();
        let wake_io = wake.clone();
        let generation = open_gen.clone();
        let stop_io = stop.clone();
        let shutdown_io = shutdown.clone();
        let closing_io = closing.clone();
        let pending_io = imports_pending.clone();
        supervisor.spawn("storage", move || {
            io_worker::run(
                data,
                io_worker::IoWorker {
                    rx: tasks,
                    tx: event_tx,
                    wake: wake_io,
                    open_gen: generation,
                    stop: stop_io,
                    shutdown: shutdown_io,
                    closing: closing_io,
                    imports_pending: pending_io,
                },
            )
        });
        let cache = paths.cache;
        let (maintenance, maintenance_rx) = mpsc::sync_channel::<()>(1);
        let cache_dir = cache.clone();
        let stopped = stop.clone();
        supervisor.spawn("cache", move || {
            while !stopped.load(Ordering::Acquire) {
                match maintenance_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(()) => {
                        let _ = crate::infrastructure::disk_cache::trim_cache(
                            &cache_dir,
                            crate::render::DISK_BUDGET,
                        );
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(_) => break,
                }
            }
        });
        let _ = maintenance.try_send(());
        let foreground = pdf_worker::Lane {
            stop: stop.clone(),
            wake: wake.clone(),
            permits: Arc::new(pdf_worker::ProcessGate::default()),
        };
        let page_prepare = prepare.clone();
        let lane = foreground.clone();
        let slot = pages.clone();
        let generation = page_gen.clone();
        let dir = cache.clone();
        supervisor.spawn("pages", move || {
            pdf_worker::pages(
                slot,
                generation,
                dir,
                page_results,
                maintenance,
                lane,
                page_prepare,
            )
        });
        let lane = foreground.clone();
        let generation = link_gen.clone();
        supervisor.spawn("links", move || {
            pdf_worker::links(links, generation, link_results, lane)
        });
        let lane = foreground.clone();
        let generation = thumb_gen.clone();
        let dir = cache.clone();
        supervisor.spawn("thumbnails", move || {
            pdf_worker::thumbnails(thumbs, generation, dir, thumb_results, lane, prepare)
        });
        let lane = foreground;
        let slot = opens.clone();
        let generation = open_gen.clone();
        let task_tx = tx.clone();
        let event_tx = events.clone();
        supervisor.spawn("metadata", move || {
            pdf_worker::opens(slot, generation, task_tx, event_tx, lane)
        });
        let stop_import = stop.clone();
        let wake_import = wake;
        let event_tx = events;
        let cancel_import = import_cancel.clone();
        let pending_import = imports_pending.clone();
        supervisor.spawn("imports", move || {
            while !stop_import.load(Ordering::Acquire) {
                let (sources, root, destination) =
                    match import_jobs.recv_timeout(Duration::from_millis(50)) {
                        Ok(job) => job,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(_) => break,
                    };
                let library = crate::services::library::LibraryService::new(
                    crate::infrastructure::filesystem::LocalFilesystem,
                );
                let event = match library.import(sources, &root, &destination, &|| {
                    cancel_import.load(Ordering::Acquire) || stop_import.load(Ordering::Acquire)
                }) {
                    Ok(count) => Event::Imported(count),
                    Err(e) => Event::Error(e),
                };
                pending_import.fetch_sub(1, Ordering::AcqRel);
                if !deliver(&event_tx, event, &stop_import, &wake_import) {
                    break;
                }
            }
        });
        Self {
            tx,
            rx,
            page_rx,
            link_tx,
            link_rx,
            link_gen,
            thumb_tx,
            thumb_rx,
            pages,
            page_gen,
            thumb_gen,
            open_gen,
            opens,
            imports,
            supervisor,
            shutdown,
            closing,
            import_cancel,
            imports_pending,
        }
    }
    pub fn submit(&self, task: Task) -> Result<(), TrySendError<Task>> {
        if let Task::Shutdown(position) = task {
            self.closing.store(true, Ordering::Release);
            self.import_cancel.store(true, Ordering::Release);
            self.cancel_pdf();
            self.shutdown.replace(position);
            return Ok(());
        }
        if self.closing.load(Ordering::Acquire) {
            return Err(TrySendError::Full(task));
        }
        match task {
            Task::Open(path, recovery, id) => {
                if self.opens.replace((path.clone(), recovery, id)) {
                    Ok(())
                } else {
                    Err(TrySendError::Disconnected(Task::Open(path, recovery, id)))
                }
            }
            Task::Import(sources, root, destination) => {
                // The configured root is validated on the import worker too.
                self.imports_pending.fetch_add(1, Ordering::AcqRel);
                self.imports
                    .try_send((sources, root, destination))
                    .map_err(|e| {
                        self.imports_pending.fetch_sub(1, Ordering::AcqRel);
                        match e {
                            TrySendError::Full((sources, root, destination)) => {
                                TrySendError::Full(Task::Import(sources, root, destination))
                            }
                            TrySendError::Disconnected((sources, root, destination)) => {
                                TrySendError::Disconnected(Task::Import(sources, root, destination))
                            }
                        }
                    })
            }
            other => self.tx.try_send(other),
        }
    }
    pub fn finish_shutdown(&mut self) {
        self.cancel_pdf();
        self.pages.close();
        self.opens.close();
        self.supervisor.complete_shutdown();
    }
    pub fn resume_after_failed_close(&self) {
        self.closing.store(false, Ordering::Release);
        self.import_cancel.store(false, Ordering::Release);
    }
    pub fn cancel_pdf(&self) {
        for generation in [
            &self.page_gen,
            &self.link_gen,
            &self.thumb_gen,
            &self.open_gen,
        ] {
            generation.fetch_add(1, Ordering::AcqRel);
        }
    }
}
impl<P> Drop for RuntimeClient<P> {
    fn drop(&mut self) {
        self.supervisor.stop.store(true, Ordering::Release);
        for generation in [
            &self.page_gen,
            &self.link_gen,
            &self.thumb_gen,
            &self.open_gen,
        ] {
            generation.fetch_add(1, Ordering::AcqRel);
        }
        self.pages.close();
        self.opens.close();
    }
}

/// Workers may backpressure, but periodically observe stop even with a full result queue.
pub(crate) fn deliver<T>(
    tx: &SyncSender<T>,
    mut value: T,
    stop: &AtomicBool,
    wake: &WakeUi,
) -> bool {
    let mut notified_full = false;
    loop {
        if stop.load(Ordering::Acquire) {
            return false;
        }
        match tx.try_send(value) {
            Ok(()) => {
                wake();
                return true;
            }
            Err(TrySendError::Full(v)) => {
                value = v;
                if !notified_full {
                    wake();
                    notified_full = true;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(TrySendError::Disconnected(_)) => return false,
        }
    }
}

#[cfg(test)]
impl<P: Send + 'static> RuntimeClient<P> {
    pub(crate) fn detached() -> (Self, Receiver<Task>) {
        let (tx, tasks) = mpsc::sync_channel(64);
        let (events, rx) = mpsc::sync_channel(64);
        let (_, page_rx) = mpsc::sync_channel(1);
        let (link_tx, _) = mpsc::sync_channel(12);
        let (_, link_rx) = mpsc::sync_channel(8);
        let (thumb_tx, _) = mpsc::sync_channel(12);
        let (_, thumb_rx) = mpsc::sync_channel(4);
        let (imports, _) = mpsc::sync_channel(1);
        let runtime = Self {
            tx,
            rx,
            page_rx,
            link_tx,
            link_rx,
            thumb_tx,
            thumb_rx,
            pages: Arc::new(Latest::default()),
            opens: Arc::new(Latest::default()),
            imports,
            page_gen: Arc::new(AtomicU64::new(0)),
            link_gen: Arc::new(AtomicU64::new(0)),
            thumb_gen: Arc::new(AtomicU64::new(0)),
            open_gen: Arc::new(AtomicU64::new(0)),
            supervisor: lifecycle::Supervisor::new(events, Arc::new(|| {})),
            shutdown: Arc::new(Latest::default()),
            closing: Arc::new(AtomicBool::new(false)),
            import_cancel: Arc::new(AtomicBool::new(false)),
            imports_pending: Arc::new(AtomicUsize::new(0)),
        };
        (runtime, tasks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::reading_position::ReadingPosition, infrastructure::storage::sqlite::Store,
    };
    #[test]
    fn full_result_queue_can_be_stopped_without_ui_drain() {
        let (tx, _rx) = mpsc::sync_channel(1);
        tx.send(1).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let (woke, wakes) = mpsc::channel();
        let wake: WakeUi = Arc::new(move || {
            let _ = woke.send(());
        });
        let handle = std::thread::spawn(move || deliver(&tx, 2, &stopped, &wake));
        wakes.recv_timeout(Duration::from_secs(2)).unwrap();
        stop.store(true, Ordering::Release);
        assert!(!handle.join().unwrap());
    }
    #[test]
    fn close_signal_bypasses_full_command_queue_and_commits_last_position() {
        let dir = tempfile::tempdir().unwrap();
        let document = dir.path().join("book.pdf");
        let (tx, tasks) = mpsc::sync_channel(64);
        let (events, rx) = mpsc::sync_channel(64);
        for page in 1..=64 {
            tx.try_send(Task::Position(
                document.clone(),
                ReadingPosition::new(page, 0.0),
                false,
            ))
            .unwrap();
        }
        assert!(matches!(
            tx.try_send(Task::Dark(true)),
            Err(TrySendError::Full(_))
        ));
        let final_position = ReadingPosition::content(65, 0.6, 1.8, 0.2);
        let shutdown = Arc::new(Latest::default());
        shutdown.replace(Some((document.clone(), final_position)));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let data = dir.path().to_path_buf();
        let handle = std::thread::spawn(move || {
            io_worker::run(
                Some(data),
                io_worker::IoWorker {
                    rx: tasks,
                    tx: events,
                    wake: Arc::new(|| {}),
                    open_gen: Arc::new(AtomicU64::new(0)),
                    stop: worker_stop,
                    shutdown,
                    closing: Arc::new(AtomicBool::new(true)),
                    imports_pending: Arc::new(AtomicUsize::new(0)),
                },
            )
        });
        loop {
            match rx.recv_timeout(Duration::from_secs(5)).unwrap() {
                Event::StorageCommitted => break,
                Event::Fatal(e) | Event::Error(e) | Event::ShutdownFailed(e) => panic!("{e}"),
                _ => {}
            }
        }
        stop.store(true, Ordering::Release);
        handle.join().unwrap();
        let mut store = Store::open(&dir.path().join("folio.sqlite3")).unwrap();
        assert_eq!(store.reading_position(&document).unwrap(), final_position);
        assert_eq!(store.take_pending().unwrap(), None);
    }
    #[test]
    fn shutdown_acknowledges_only_after_workers_release_resources() {
        let dir = tempfile::tempdir().unwrap();
        let mut runtime = RuntimeClient::start(
            AppPaths {
                data: Some(dir.path().to_path_buf()),
                cache: dir.path().join("cache"),
            },
            Arc::new(|| {}),
            Arc::new(|pixels: crate::domain::pixels::Pixels| pixels),
        );
        runtime.submit(Task::Shutdown(None)).unwrap();
        loop {
            match runtime.rx.recv_timeout(Duration::from_secs(5)).unwrap() {
                Event::StorageCommitted => runtime.finish_shutdown(),
                Event::Closed => break,
                Event::Fatal(e) | Event::ShutdownFailed(e) => panic!("{e}"),
                _ => {}
            }
        }
        let _lock =
            crate::infrastructure::platform::InstanceLock::acquire(&dir.path().join("folio.lock"))
                .unwrap();
    }
    #[test]
    fn runtime_shutdown_submission_remains_available_when_saturated() {
        let (runtime, _tasks) = RuntimeClient::<crate::domain::pixels::Pixels>::detached();
        for _ in 0..64 {
            runtime.submit(Task::Dark(true)).unwrap();
        }
        assert!(matches!(
            runtime.submit(Task::Dark(false)),
            Err(TrySendError::Full(_))
        ));
        assert!(runtime.submit(Task::Shutdown(None)).is_ok());
        assert!(runtime.shutdown.try_take().is_some());
    }
}

#[cfg(test)]
mod annotation_workflow_tests {
    use super::*;
    use crate::domain::annotation::Annotation;

    #[test]
    fn accepted_annotations_are_committed_before_export_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let document = dir.path().join("book.pdf");
        let destination = dir.path().join("notes.md");
        let mut runtime = RuntimeClient::start(
            AppPaths {
                data: Some(dir.path().into()),
                cache: dir.path().join("cache"),
            },
            Arc::new(|| {}),
            Arc::new(|pixels: crate::domain::pixels::Pixels| pixels),
        );
        let item = Annotation {
            id: 0,
            page: 7,
            text: "Selected words".into(),
            note: "My note\nNext line".into(),
            color: Some(2),
            bounds: vec![[0.1, 0.2, 0.3, 0.4]],
        };
        runtime
            .submit(Task::SaveAnnotation(document.clone(), Box::new(item)))
            .unwrap();
        runtime
            .submit(Task::ExportAnnotations(
                document.clone(),
                destination.clone(),
            ))
            .unwrap();
        let mut saved_id = None;
        loop {
            match runtime.rx.recv_timeout(Duration::from_secs(5)).unwrap() {
                Event::Annotations(path, items) => {
                    assert_eq!(path, document);
                    saved_id = Some(items[0].id);
                }
                Event::Notice(message) if message == "Annotations exported." => break,
                Event::Error(e) | Event::Fatal(e) => panic!("{e}"),
                _ => {}
            }
        }
        let export = std::fs::read_to_string(destination).unwrap();
        assert!(export.contains("Page 7 — Highlight"));
        assert!(export.contains("> Selected words"));
        assert!(export.contains("My note\nNext line"));
        runtime
            .submit(Task::DeleteAnnotation(document, saved_id.unwrap()))
            .unwrap();
        loop {
            match runtime.rx.recv_timeout(Duration::from_secs(5)).unwrap() {
                Event::Annotations(_, items) => {
                    assert!(items.is_empty());
                    break;
                }
                Event::Error(e) | Event::Fatal(e) => panic!("{e}"),
                _ => {}
            }
        }
        runtime.submit(Task::Shutdown(None)).unwrap();
        loop {
            match runtime.rx.recv_timeout(Duration::from_secs(5)).unwrap() {
                Event::StorageCommitted => runtime.finish_shutdown(),
                Event::Closed => break,
                Event::Fatal(e) | Event::ShutdownFailed(e) => panic!("{e}"),
                _ => {}
            }
        }
    }
}
