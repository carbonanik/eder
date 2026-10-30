use super::*;
use crate::{
    app::state::View,
    domain::{document::Entry, reading_position::ReadingPosition},
    infrastructure::{filesystem::LocalFilesystem, storage::sqlite::Store},
    services::{library::LibraryService, persistence::PersistenceService},
};
pub(super) struct IoWorker {
    pub rx: Receiver<Task>,
    pub tx: SyncSender<Event>,
    pub wake: WakeUi,
    pub open_gen: Arc<AtomicU64>,
    pub stop: Arc<AtomicBool>,
    pub shutdown: Arc<Latest<Option<(PathBuf, ReadingPosition)>>>,
    pub closing: Arc<AtomicBool>,
    pub imports_pending: Arc<AtomicUsize>,
}
pub(super) fn run(data: Option<PathBuf>, worker: IoWorker) {
    let IoWorker {
        rx,
        tx,
        wake,
        open_gen,
        stop,
        shutdown,
        closing,
        imports_pending,
    } = worker;
    let send = |event| {
        deliver(&tx, event, &stop, &wake);
    };
    let path = match data {
        Some(data) => match std::fs::create_dir_all(&data) {
            Ok(()) => data.join("folio.sqlite3"),
            Err(e) => {
                send(Event::Fatal(format!(
                    "Cannot create local data folder: {e}"
                )));
                return;
            }
        },
        None => {
            send(Event::Fatal(
                "No local user-data directory is available.".into(),
            ));
            return;
        }
    };
    let _instance_lock = match crate::infrastructure::platform::InstanceLock::acquire(
        &path.with_extension("lock"),
    ) {
        Ok(lock) => lock,
        Err(error) => {
            send(Event::Fatal(error));
            return;
        }
    };
    let mut store = match Store::open(&path) {
        Ok(s) => PersistenceService::new(s),
        Err(e) => {
            send(Event::Fatal(format!("Cannot open local database: {e}")));
            return;
        }
    };
    let library = LibraryService::new(LocalFilesystem);
    match store.snapshot() {
        Ok(s) => send(Event::Snapshot(s)),
        Err(e) => send(Event::Error(e)),
    }
    match store.take_pending() {
        Ok(Some(p)) => send(Event::Recovery(p)),
        Ok(None) => {}
        Err(e) => send(Event::Error(e)),
    }
    while !stop.load(Ordering::Acquire) {
        let task = match rx.try_recv() {
            Ok(task) => task,
            Err(mpsc::TryRecvError::Empty) => {
                if imports_pending.load(Ordering::Acquire) == 0 {
                    if let Some(position) = shutdown.try_take() {
                        Task::Shutdown(position)
                    } else {
                        match rx.recv_timeout(Duration::from_millis(25)) {
                            Ok(task) => task,
                            Err(mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(_) => break,
                        }
                    }
                } else {
                    match rx.recv_timeout(Duration::from_millis(25)) {
                        Ok(task) => task,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(_) => break,
                    }
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
        };
        let failed_open = if let Task::Opened(_, _, id, _, _, _) = &task {
            Some(*id)
        } else {
            None
        };
        let failed_listing = if let Task::Load(id, _, _) = &task {
            Some(*id)
        } else {
            None
        };
        let result: Result<(), String> = (|| {
            match task {
                Task::SaveAnnotation(path, item) => {
                    store.save_annotation(&path, &item)?;
                    send(Event::Annotations(path.clone(), store.annotations(&path)?));
                    return Ok(());
                }
                Task::DeleteAnnotation(path, id) => {
                    store.delete_annotation(&path, id)?;
                    send(Event::Annotations(path.clone(), store.annotations(&path)?));
                    return Ok(());
                }
                Task::ExportAnnotations(path, destination) => {
                    let text =
                        crate::domain::annotation::export_markdown(&store.annotations(&path)?);
                    std::fs::write(destination, text).map_err(|e| e.to_string())?;
                    send(Event::Notice("Annotations exported.".into()));
                    return Ok(());
                }
                Task::Load(id, view, folder) => {
                    let s = store.snapshot()?;
                    let root = s.root.clone();
                    let rows = match view {
                        View::Library => match (s.root, folder) {
                            (Some(root), Some(folder)) => library.scan(&root, &folder),
                            _ => Ok(vec![]),
                        },
                        View::Reading => Ok(s.history.into_iter().map(Entry::file).collect()),
                        View::Favorites => {
                            let mut rows: Vec<_> =
                                s.favorites.into_iter().map(Entry::file).collect();
                            rows.sort_by_cached_key(|e| e.name.to_lowercase());
                            Ok(rows)
                        }
                        View::Settings => Ok(vec![]),
                    };
                    let folders = root
                        .as_ref()
                        .and_then(|root| library.scan(root, root).ok())
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|entry| entry.directory)
                        .collect();
                    send(Event::Rows(id, rows));
                    send(Event::Collections(id, folders));
                    return Ok(());
                }
                Task::Import(..) | Task::Open(..) => unreachable!("routed to dedicated workers"),
                Task::Root(p) => {
                    store.root(&p)?;
                    send(Event::Notice("Library folder updated.".into()));
                }
                Task::Favorite(p, b) => {
                    store.favorite(&p, b)?;
                    send(Event::Notice(
                        if b {
                            "Added to Favorites."
                        } else {
                            "Removed from Favorites."
                        }
                        .into(),
                    ));
                }
                Task::Remove(p) => {
                    store.remove_history(&p)?;
                    send(Event::Notice(
                        "Removed from Currently Reading. The PDF is unchanged.".into(),
                    ));
                }
                Task::Opened(p, recovery, id, total, geometry, revision) => {
                    if open_gen.load(Ordering::Acquire) != id {
                        return Ok(());
                    }
                    let saved = store.reading_position(&p)?;
                    let position = ReadingPosition {
                        page: saved.page.clamp(1, total),
                        ..saved
                    };
                    store.opened(&p, recovery)?;
                    // Do not overwrite the scroll fraction on open: this must survive
                    // until the first restored viewport frame is shown.
                    send(Event::Open(
                        p.clone(),
                        position,
                        total,
                        id,
                        geometry,
                        revision,
                    ));
                    send(Event::Annotations(p.clone(), store.annotations(&p)?));
                }
                Task::Position(p, position, explicit) => {
                    store.save_reading_position(&p, position)?;
                    if explicit {
                        let suffix = if position.fraction > 0.001 {
                            format!(" ({:.0}% through page)", position.fraction * 100.0)
                        } else {
                            String::new()
                        };
                        send(Event::Notice(format!(
                            "Position saved: page {}{suffix}.",
                            position.page
                        )));
                    }
                    return Ok(());
                }
                Task::DarkTheme(b) => {
                    store.dark_theme(b)?;
                    send(Event::DarkTheme(b));
                    return Ok(());
                }
                Task::Dark(b) => {
                    store.dark(b)?;
                    send(Event::Dark(b));
                    return Ok(());
                }
                Task::ReaderPanels(annotations, pages) => {
                    store.reader_panels(annotations, pages)?;
                    return Ok(());
                }
                Task::ReadingMode(b) => {
                    store.continuous_scroll(b)?;
                    send(Event::ReadingMode(b));
                    return Ok(());
                }
                Task::Close(path, position, id) => {
                    match store.finish_session(Some((&path, position))) {
                        Ok(()) => send(Event::ReaderClosed(id)),
                        Err(error) => send(Event::ReaderCloseFailed(id, error)),
                    }
                    return Ok(());
                }
                Task::Shutdown(position) => {
                    if let Err(e) = store.finish_session(
                        position
                            .as_ref()
                            .map(|(path, position)| (path.as_path(), *position)),
                    ) {
                        closing.store(false, Ordering::Release);
                        send(Event::ShutdownFailed(e));
                        return Ok(());
                    }
                    send(Event::StorageCommitted);
                    return Ok(());
                }
            }
            send(Event::Snapshot(store.snapshot()?));
            Ok(())
        })();
        if let Err(e) = result {
            if let Some(id) = failed_open {
                send(Event::OpenFailed(id, e));
            } else if let Some(id) = failed_listing {
                send(Event::Rows(id, Err(e)));
            } else {
                send(Event::Error(e));
            }
        }
    }
}
