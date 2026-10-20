# Eder desktop architecture plan

Status: original implementation plan, 20 October 2026. The structural refactor and reader/persistence changes have now been implemented. See [the implemented architecture](architecture.md) for final module boundaries and policy choices, and [validation](architecture-validation.md) for evidence and remaining desktop checks. Proposed performance targets below remain acceptance targets rather than measured guarantees.

## 1. Decision and scope

Keep one Rust application and one Cargo package. Use a modular monolith with a main-thread application controller, synchronous use-case services executed by supervised workers, and typed results returned to the controller. Keep eframe/egui, SQLite, and the existing Poppler backend.

The supplied recommendation has the right overall direction. It needs explicit rules for ownership, module dependencies, queue saturation, stale results, resource limits, and shutdown to become an implementable architecture. Creating folders alone will not establish those rules.

This plan applies to Eder, the application in this repository, previously named Folio. It preserves existing data locations and behavior. Bookmarks, annotations, full-text indexing, multiple tabs, and a new PDF backend remain separate features. Existing import and filename filtering behavior must also survive the refactor, even though older requirements describe a smaller feature set.

## 2. Current implementation and reasons to change

Repository inspection found:

- `src/app.rs` is approximately 2,900 lines and combines widgets, state, task handling, worker startup, rendering scheduling, texture caching, and shutdown.
- `src/library.rs`, `src/store.rs`, and `src/render.rs` already separate some filesystem, SQLite, and PDF operations.
- Background work, bounded channels, cancellation generations, resolution buckets, limited texture uploads, and library/reader virtualization already exist. Preserve them before improving their contracts.
- Poppler runs through cancellable child processes. An in-process Pdfium threading policy does not describe the current implementation.
- Continuous mode uses equal-height rows based on an assumed aspect ratio. Mixed page sizes need actual geometry for natural layout and reliable anchors.
- Reading positions currently store a one-based page and a fraction of its row stride. Zoom is not part of the stored position.
- Workers are spawned without a central supervisor retaining their join handles. Some result polling drains a channel fully in one frame.
- Documentation resource limits have drifted: the code currently uses a 160 MiB page budget and a 384 MiB disk budget, while the README describes older values. Central policy and updated documentation should eliminate this discrepancy.

These are structural findings, not a diagnosis proving the cause of every reported frame drop. Record representative frame and rendering measurements before changing the pipeline.

## 3. Module boundaries and dependency rules

```text
main + desktop shell: construct and connect the parts
         |
         +--> ui --> app --> domain
         |           ^
         +--> runtime+      (runtime executes app effects and returns events)
                 |
                 +--> services --> domain
                 |       |
                 |       +--> service ports
                 +--> infrastructure --> service ports + domain
```

Arrows represent permitted code dependencies. Runtime and application communicate through protocols defined by `app`; application code does not import the runtime. The composition root supplies infrastructure implementations to services.

| Module | Responsibility | Excludes |
| --- | --- | --- |
| `domain` | Document identity, validated positions, page geometry, settings, links | egui, SQLite, subprocesses, channels |
| `app` | Feature state, action/event transitions, navigation, render demand, save scheduling | Filesystem access, SQL, PDF execution, textures |
| `ui` | Widgets, theme, local editing state, textures and upload scheduling | Worker startup, persistence, PDF processing |
| `services` | Blocking use cases such as scanning a library, loading a document, and persisting state; narrow ports | Widgets, repaint calls, channel scheduling |
| `runtime` | Worker ownership, effect dispatch, cancellation, queues, result delivery and wakeups | Product navigation rules and widgets |
| `infrastructure` | Poppler, SQLite, filesystem, disk cache and native platform adapters | Application state transitions |

Application effects describe work, not service objects. For example, `OpenDocument` becomes document-service work in the runtime. Completion becomes an `AppEvent` applied by the controller. Small pure reader calculations belong in `app/reader`; they do not need a service wrapper.

Use `pub(crate)` and private module internals by default. Introduce traits for replaceable or testable boundaries such as `PdfEngine` and `LibraryRepository`; avoid a trait and wrapper for every struct. Do not require engine handles to implement `Send + Sync`: construct and retain them inside their owning worker if necessary. No generic service registry, dependency injection framework, or global event bus is needed.

## 4. Target source layout

```text
src/
  main.rs                       # window configuration and composition root
  lib.rs                        # headless application core and module exports
  desktop.rs                    # eframe adapter; frame orchestration
  domain/
    mod.rs
    document.rs                 # DocumentId, FileRevision, PageNumber, geometry, links
    reading_position.rs
    settings.rs
    error.rs
  app/
    mod.rs                      # controller and transition entry points
    state.rs                    # LibraryState, ReaderState, SettingsState, lifecycle
    action.rs                   # user intent and viewport reports
    effect.rs                   # typed work descriptions
    event.rs                    # correlated results and worker failures
    library.rs                  # browsing and collection transitions
    reader/
      mod.rs                    # reader transitions
      layout.rs                 # page offsets, visible range and anchors
      scheduler.rs              # desired render set and priority
  ui/
    mod.rs
    library.rs
    reader.rs
    settings.rs
    navigation.rs
    theme.rs
    textures.rs                 # main-thread LRU and bounded uploads
  services/
    mod.rs
    ports.rs                    # PDF, repository and filesystem contracts
    library.rs
    document.rs
    persistence.rs
  runtime/
    mod.rs                      # runtime client and worker supervisor
    queues.rs                   # bounded queues and latest-value mailboxes
    io_worker.rs
    pdf_worker.rs
    thumbnail_worker.rs
    lifecycle.rs
  infrastructure/
    mod.rs
    pdf/
      mod.rs
      poppler.rs
      process.rs
    storage/
      mod.rs
      sqlite.rs
      migrations.rs
    filesystem.rs
    disk_cache.rs
    platform.rs                 # dialogs, data paths and wakeup adapter
```

This is the target map, not an instruction to create empty modules. Extract modules when moving real behavior. Keep a single package until a concrete reuse or build constraint warrants multiple crates. Add a search service and index worker when full-text search is actually implemented.

## 5. State and ownership

The desktop shell owns `Controller`, `UiState`, `TextureCache`, and `RuntimeClient`. A runtime supervisor owns worker handles and shutdown coordination.

- The controller exclusively mutates application state on the UI thread. Use feature substates and enums such as `ReaderStatus::{Closed, Opening, Ready, Failed}` and `Lifecycle::{Starting, Running, Closing, Failed}` instead of overlapping booleans.
- The UI owns focus, drafts, panels, egui scroll state, and texture handles. Its scroll offset is a presentation value; the controller owns the document anchor derived from it.
- The storage worker exclusively owns its SQLite connection. Services receive access on that worker; the UI never shares it through a mutex.
- PDF workers own child processes and decoding buffers. Results carry owned pixels and domain metadata, not engine handles or UI objects.
- Synchronization protects only queues, cancellation and short-lived transport state. Do not put `AppState` behind `Arc<Mutex<_>>`.
- Persistent state has an acknowledged in-memory projection. A queued favorite or save is pending until its transaction succeeds. Failures leave a visible retryable state.

Maintain one source of truth for each datum. Derived counts, search indices and visible page sets must be explicitly invalidated by their source revision.

## 6. Actions, effects and results

Use a small transition interface:

```text
Controller::handle(Action or AppEvent) -> bounded collection of Effects
RuntimeClient::submit(Effect) -> Accepted | Busy(effect) | Disconnected(effect)
```

Actions include opening a document, navigating a folder, changing zoom, reporting a viewport, and saving a position. Events include document metadata, page pixels, persisted settings, and operation failures. Effects include reads, renders, writes and cancellation. Cheap local actions are ordinary calls; only work crossing a worker boundary needs messaging.

Every asynchronous result includes an operation ID and its relevant scope:

- Folder loads: listing generation and requested folder.
- Document open: open request ID and document identity.
- Page render: reader session ID, render generation and full render key.
- Persistence: document identity, operation ID and monotonic save sequence.

A render key includes document identity, file revision, page number, resolution bucket, rotation, pixel transform and backend/cache version. A session ID identifies an opening of a document; it is different from persistent document identity. Reopening the same file must not accept results from the previous session.

Discard obsolete work before execution where possible and validate results again before changing state or uploading textures. Cancellation is an expected outcome, not a document failure. Never invalidate persistence writes merely because the user opened a different document.

Example: opening B while A loads increments the open request ID. A's late metadata cannot replace B. A's already accepted reading-position save still commits for A.

## 7. Workers, queue policy and repainting

Start with three active work lanes: one I/O worker for short database/filesystem operations, one foreground PDF worker for metadata/geometry/page/link work, and one thumbnail worker. Retain the existing separate link lane initially during extraction; only consolidate it after measuring foreground latency. A maintenance worker handles cache trimming. Long import copies run as bounded, cancellable chunks or on a separate single-job worker so that saves cannot wait behind an entire import.

Poppler subprocess concurrency is explicitly capped, initially at two across PDF-related lanes, with foreground work preferred. A thumbnail already running may finish or be cancelled when reader demand arrives. The cap includes metadata/link subprocesses. Retain subprocess timeouts and kill/reap on cancellation.

The capacities below are proposed starting policies, subject to measurement:

| Traffic | Bound and saturation behavior |
| --- | --- |
| Viewport/render demand | One replaceable latest demand set; contains visible pages and at most three neighboring pages in each direction |
| Foreground raster results | One completed pixel buffer plus one in-flight render; enforce a byte limit as well |
| Thumbnail work/results | 12 requests, four results; drop obsolete requests and retry visible demand later |
| Durable mutations | 64 accepted commands; return `Busy` if full; never silently discard accepted writes |
| Automatic positions | Latest unsent value per document, at most eight distinct pending documents; persist before allowing more document transitions if full |
| Control results | 64 small messages, ordered and lossless for acknowledged writes; separate from pixel results |
| Shutdown | Dedicated lifecycle signal and final-save barrier, independent of a full ordinary queue |

An explicit save is ordered after the latest automatic position for that document and carries a sequence. Coalescing must never let an older position overwrite a newer accepted save. If a critical action is busy, retain a bounded pending action or reject it visibly; do not grow an unbounded retry list.

All UI submissions are nonblocking. Bounded `sync_channel` alone is insufficient because `send` blocks when full; use `try_send` or a mailbox with explicit replacement rules. See the [Rust bounded-channel contract](https://doc.rust-lang.org/std/sync/mpsc/fn.sync_channel.html).

Workers enqueue results and trigger a small `WakeUi` callback. The desktop implementation calls `egui::Context::request_repaint`; services remain unaware of egui. This wakeup is supported by the [pinned egui context implementation](https://github.com/emilk/egui/blob/0.29.1/crates/egui/src/context.rs). The UI requests another frame whenever its processing budget leaves results pending, so it cannot depend on a future worker wakeup to drain an existing backlog.

Initially process at most 32 small control events or about 2 ms per frame, plus at most one page upload or one thumbnail upload. Actual upload duration must be measured; a time check cannot interrupt an upload already started. Timed autosaves use delayed repaints. Idle frames do not poll continuously.

## 8. PDF layout, rendering and caches

Keep Poppler behind `PdfEngine` operations for metadata, page geometry, page rasterization and links. Specify page crop box, rotation and coordinate conventions together. The backend returns geometry without rasterizing every page. For large documents, metadata may arrive in bounded batches; preserve the current anchor as estimated geometry is replaced.

Use each page's aspect ratio and rotation to compute its displayed rectangle. Maintain page start offsets and binary-search them for the viewport intersection. Reserve the full document extent, but instantiate widgets only for visible pages and a small margin. `show_rows` remains suitable for uniform library rows; variable page heights should use viewport-based layout. Both capabilities exist in the [egui 0.29.1 scroll-area source](https://github.com/emilk/egui/blob/0.29.1/crates/egui/src/containers/scroll_area.rs).

Render visible missing pages first, directional neighbors second, and thumbnails last. Deduplicate by render key. Resize/zoom demand uses resolution buckets, keeping a usable lower-resolution texture until a replacement arrives. Avoid restarting a still-useful render on every scroll delta.

Separate resources and accounting:

- Main-thread page textures: initially retain the current 160 MiB estimated RGBA budget.
- Main-thread thumbnails: at most 64 entries at up to 192 pixels on the longest edge, with byte accounting too.
- CPU results/in-flight buffers: separately bounded; do not retain a second full decoded-page cache without measured benefit.
- Disk-derived images: initially retain the current 384 MiB policy; cleanup runs off foreground workers.
- Link/geometry metadata: separately bounded or accounted; do not assume non-pixel caches are free.

Keep the current 2,400-pixel raster cap during the structural refactor. Change resolution quality only in a measured follow-up. A true LRU updates recency on use, not only insertion. If visible textures cannot fit the budget, lower raster resolution or use placeholders; unlimited pinning must not silently defeat the limit. Drop every retained texture handle when evicting, including aliases in reader state.

Resource limits bound retained application resources, not total RSS or VRAM. GPU copies, temporary conversion buffers and Poppler processes add memory. Disk writes use temporary files and atomic publication where supported; trimming excludes active temporary files. Cache failures fall back to rendering and never fail a successful reading-position save.

## 9. Reading position and persistence

Keep document identity based on native canonical paths for the initial refactor, matching existing behavior. Wrap it in `DocumentId` so path representation does not spread. File moves continue to produce a different identity; transparent rename tracking requires a separate product decision. File revision is a separate size/mtime-based cache invalidator, with explicit refresh invalidation for replacements that preserve metadata.

Retain a validated one-based `PageNumber` in the domain, matching current storage and Poppler. Convert to zero-based array indices at a single checked boundary. This avoids unnecessary page-number migration.

The target reading position contains document identity, page, normalized vertical offset in the page's displayed content, zoom policy/value, and normalized horizontal pan when applicable. Define the anchor as the page point at the top of the viewport. If the viewport starts in a gap, use the next page's top. On resize or zoom, map the same page point into the new geometry and clamp to the document's scroll range. Link destinations use the same coordinates. Keep reading mode in preferences unless per-document mode becomes a requirement.

Do not reinterpret the existing row-stride fraction as a content fraction. Preserve the old value under a legacy anchor version, reconstruct its old position when geometry is available, then write the new version on the next successful save. Add defaults for previously unstored zoom/pan.

Keep preferences in SQLite alongside existing state. A separate JSON/TOML file adds no current benefit and creates another consistency boundary. Keep the existing `com / Folio / Folio` data directory, `folio.sqlite3`, native path encoding, favorites, MRU and recovery semantics.

Use explicit schema versions and transactional migrations. Debounce automatic position writes, initially preserving 650 ms, but force the latest position on document switch, Back, explicit Save and close. Confirm explicit Save only after commit. Use the existing WAL/FULL policy initially and preserve one-shot recovery behavior with migration fixtures. Enforce one active instance per data directory, or visibly refuse a second instance, before claiming coordinated recovery ownership.

## 10. Errors and lifecycle

Use structured errors for I/O, storage, invalid/unsupported PDF, missing backend, timeout, cancellation and worker unavailability. Keep operation/document context and underlying causes. Convert to user-facing text at the application/UI boundary. Cache errors are recoverable; storage errors affect durability and must be visible. Avoid using strings to decide whether an error was cancellation.

Startup loads/migrates storage on the I/O worker while the shell displays a startup state. Failure exposes a usable error screen. Worker panics or disconnected channels become explicit runtime events rather than an infinite spinner.

On close: reject new actions, capture the latest position, cancel speculative PDF work, commit accepted writes and the final position, then clear recovery in an ordered storage barrier. Only after acknowledgment should the window close. If storage fails, keep the window open with retry or explicit exit choices.

The supervisor retains worker handles, wakes every waiting mailbox on stop, and ensures cancelled child processes are killed and reaped. The UI continues draining required acknowledgments during shutdown. A non-UI coordinator joins workers; never block an egui frame waiting on a worker whose result channel requires UI draining. A shutdown deadline reports a stalled worker without pretending uncommitted data was saved.

## 11. Incremental implementation order

Each phase leaves a working application. Keep structural changes separate from behavior changes.

| Phase | Work | Completion evidence |
| --- | --- | --- |
| 0. Baseline | Reconcile current behavior with older requirements; record fixtures, cache limits and representative frame/render measurements | Documented baseline and repeatable manual scenarios |
| 1. Domain and contracts | Extract positions, geometry, IDs, settings and errors; add typed action/effect/event contracts | Existing storage/render tests pass; no data or visible behavior changes |
| 2. Worker extraction | Move current worker loops into runtime; retain queue sizes and separate link work; add supervision and lifecycle transport | Saturation, cancellation, disconnect and shutdown tests; existing behavior retained |
| 3. Controller and UI | Extract feature state/transitions, pure reader logic, widgets and texture cache; reduce desktop shell to orchestration | Headless transitions run without egui; compact/wide desktop journeys remain usable |
| 4. Service boundaries | Move use cases behind narrow repository/PDF/filesystem ports; split long I/O from durable writes | Fake-engine tests and real SQLite/Poppler integration tests pass |
| 5. Reader improvements | Add actual geometry, versioned anchors, latest-demand scheduling, bounded metadata/results and byte accounting | Mixed-size PDFs, fast scroll, zoom/resize and stale-result scenarios pass |
| 6. Persistence hardening | Add explicit migrations, save barriers, single-instance policy and recovery/shutdown verification | Old database fixtures retain state; final saves commit before close |
| 7. Measure and document | Compare baseline, tune budgets from evidence, update source map and limits | Agreed performance targets met on the representative corpus; desktop smoke pass |

The first code milestone should be phases 1–2: extract domain contracts and supervised workers while preserving the current UI and renderer. This provides a reviewable foundation before touching reader layout.

## 12. Validation and acceptance

Use meaningful unit tests for transitions, visible-range geometry, anchor conversion, render priority, stale-result rejection, save ordering and queue saturation. Drive queue/lifecycle tests with deterministic fakes and a fake clock, avoiding sleeps. Test full result queues and close during an active render to catch shutdown deadlocks.

Integration tests cover old database migrations, independent document positions, commit failures, recovery, corrupt/missing files, actual Poppler geometry/links/raster output and cancellation. Maintain a small corpus with mixed portrait/landscape pages, rotation, scanned pages and complex vector content. Verify source PDFs remain unchanged except for explicitly authorized import copying.

Run `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked`; run Poppler-dependent ignored tests explicitly where available. Native desktop smoke tests remain necessary for graphics, dialogs and frame timing. These checks are planned validation, not results from this documentation-only change.

Suggested initial performance acceptance targets, to validate against named hardware and documents:

- At 60 Hz, p95 application frame work stays below 16.7 ms during cached scrolling; separately measure event handling, texture upload and presentation.
- Visible-page widget work scales with visible pages plus a fixed margin, not total page count; geometry preparation can scale with page count off the UI thread.
- Pending jobs, decoded pixel payloads, textures and metadata stay within their documented budgets during repeated scroll/zoom/open operations.
- Late results never replace a newer document session or render generation.
- Accepted writes complete in order; explicit Save and normal close acknowledge a real commit.
- Reopening and resizing mixed-size documents preserves the defined page anchor within pixel-rounding tolerance, except unavoidable clamping near document boundaries.

Record cold/warm page latency and p50/p95/p99 frame times, including thumbnail activity and slow storage. Engine replacement, async runtimes and additional parallelism should follow measured bottlenecks. They are not prerequisites for this architecture.
