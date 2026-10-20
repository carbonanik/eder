use super::*;

pub(super) struct Supervisor {
    pub stop: Arc<AtomicBool>,
    handles: Vec<JoinHandle<()>>,
    events: SyncSender<Event>,
    wake: WakeUi,
}
impl Supervisor {
    pub fn new(events: SyncSender<Event>, wake: WakeUi) -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(false)),
            handles: Vec::new(),
            events,
            wake,
        }
    }
    pub fn spawn(&mut self, name: &'static str, work: impl FnOnce() + Send + 'static) {
        let events = self.events.clone();
        let wake = self.wake.clone();
        let stop = self.stop.clone();
        self.handles.push(
            std::thread::Builder::new()
                .name(format!("eder-{name}"))
                .spawn(move || {
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).is_err() {
                        deliver(
                            &events,
                            Event::Fatal(format!(
                                "The {name} worker stopped unexpectedly. Restart Eder."
                            )),
                            &stop,
                            &wake,
                        );
                    }
                })
                .expect("worker thread could not start"),
        );
    }
}
impl Supervisor {
    pub fn complete_shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        let handles = std::mem::take(&mut self.handles);
        let events = self.events.clone();
        let wake = self.wake.clone();
        std::thread::spawn(move || {
            let mut failed = false;
            for handle in handles {
                failed |= handle.join().is_err();
            }
            wake();
            let event = if failed {
                Event::Fatal("A background worker failed during shutdown.".into())
            } else {
                Event::Closed
            };
            let _ = events.send(event);
            wake();
        });
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let handles = std::mem::take(&mut self.handles);
        if handles.is_empty() {
            return;
        }
        // Joining must never block an egui frame or wait on a result it must drain.
        std::thread::spawn(move || {
            for handle in handles {
                let _ = handle.join();
            }
        });
    }
}
