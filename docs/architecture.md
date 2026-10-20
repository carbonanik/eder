# Implemented architecture

Eder is one Rust package with an egui desktop adapter, a headless application controller, synchronous services, supervised workers, and local infrastructure. This implements the structure proposed in [the architecture plan](architecture-plan.md). Search indexing, annotations, tabs, and replacing Poppler are separate feature work.

## Source map and dependencies

| Location | Responsibility |
| --- | --- |
| `src/main.rs` | Native window and executable entry point |
| `src/desktop.rs`, `src/desktop/` | Connect controller, runtime and UI; bounded frame processing, dispatch and texture uploads |
| `src/domain/` | Document identity/revision, render keys, geometry, reading positions, persistent snapshot and PDF error types |
| `src/app/state.rs` | Main-thread controller state and explicit application lifecycle |
| `src/app/controller.rs`, `action.rs`, `effect.rs`, `event.rs` | Local actions, state transitions, work descriptions and correlated completions |
| `src/app/reader/` | Reader state, viewport geometry, binary-search visibility, anchor calculation and render priority |
| `src/ui/` | Library, reader, settings, navigation, theme, presentation state and texture LRU |
| `src/services/` | Library/document/persistence operations and backend/repository/filesystem ports |
| `src/runtime/` | Named workers, bounded channels, latest-request mailboxes, cancellation, process admission and worker joins |
| `src/infrastructure/` | Poppler processes, filesystem/imports, SQLite migrations, disk cache, OS paths/dialogs and instance lock |
| `src/policy.rs` | Shared resource limits and raster resolution buckets |
| `src/library.rs`, `render.rs`, `store.rs` | Compatibility exports for existing callers and examples |

`domain` uses standard Rust types and has no GUI, SQL, image-decoding or channel dependency. `app` depends on domain models and its own protocols. Services depend on ports/domain, with image decoding types confined to the PDF boundary. Infrastructure implements the service ports. Runtime constructs the concrete adapters on their owning workers. Desktop/UI connect to the headless core; the core never imports desktop/UI.

The existing compatibility entry points remain supported; new implementation code should use the owning module. There is no async runtime, global mutable state, service registry, empty indexing scaffold, or multi-crate workspace.

## State and frame flow

The desktop shell exclusively owns the controller, UI state and runtime client. `Controller::on_event` rejects obsolete listing/open results and returns explicit transitions. `Controller::handle` processes local reader actions with an injected timestamp. Reader calculations and scheduling can be tested without an egui context.

UI state exclusively retains texture handles, thumbnail recency, search drafts, dialogs, focus-related values and pending presentation requests. Reader application state contains geometry and navigation state, without texture handles. SQLite belongs to the storage worker; engine instances are created inside their worker and are not required to implement `Send` or `Sync`.

A frame processes up to 32 control events, one page upload and eight link results. Thumbnail uploads are limited to one per frame in the library. Consuming bounded pixel results or leaving a control backlog requests another repaint. Workers invoke an injected wake callback when producing results. The desktop adapter implements it with `request_repaint`. Pixel conversion is injected into the runtime and executed on workers, keeping egui out of services/runtime and large conversions off the frame thread.

The frame event limit is a count bound, not a hard time deadline. Native frame percentiles and GPU upload time still need measurement on a graphical desktop.

## Work and saturation policy

| Work | Bound and behavior |
| --- | --- |
| Storage commands/results | 64 each; UI submission uses `try_send`, reports busy/disconnection, and never silently drops an accepted write |
| Document open | One replaceable metadata request, plus the active operation; session generation rejects stale completion and errors |
| Page raster demand | One replaceable worker request; controller keeps only visible missing pages and three neighbors on each side |
| Page results | One completed result slot plus active rendering; each raster is capped at a 2,400-pixel longest edge |
| Thumbnail jobs/results | 12 / 4; obsolete jobs are skipped; visible requests can be retried |
| Link jobs/results | 12 / 8; only 64 pages of retained link metadata |
| Imports | One queued job plus one active import; copies check cancellation between 64 KiB chunks; destination confinement and collision protection stay on the worker |
| Shutdown | A separate latest-value lifecycle mailbox bypasses a full normal command queue; rejects new mutations while closing |
| Cache maintenance | One coalesced request; disk scans run outside foreground workers |

The named lanes are storage, metadata, foreground pages, links, thumbnails, imports and cache maintenance. Document metadata/PDF calls and imports no longer block the storage command handler. Directory enumeration remains on the I/O worker; an unusually slow filesystem can delay its later saves, while the UI remains responsive. Extracting a separate directory lane is a follow-up if measurements justify it.

PDF admission permits at most two active engine operations. Background thumbnail/link work uses at most one slot and yields admission to queued foreground work. Poppler child execution has a 30-second timeout and observes cancellation generations. Cancelling a child kills and reaps it. Result delivery periodically observes shutdown even when its bounded channel is full.

Automatic position updates are debounced for 650 ms in reader state. Unsent updates coalesce there; document switching first submits the old document's position. Writes use one ordered queue. Explicit Save confirms only after the write succeeds. A full queue leaves an automatic position dirty or rejects the user action visibly. Back keeps the reader open until its final-position/recovery transaction succeeds.

## PDF layout, identity and caches

Metadata reads the media box and intrinsic rotation, matching the existing `pdftoppm`/`pdftohtml` convention. Visible-page rectangles use actual dimensions. Layout caches page offsets and heights; it rebuilds on document geometry, width or zoom changes. Visibility lookup uses binary search; offscreen pages retain their extent without producing widgets. Continuous mode supports horizontal panning when zoomed.

A render key contains canonical native document identity, file size/nanosecond modification revision, page, resolution bucket, dark-page transform and backend convention version. Worker generations are scoped to sessions/render demand. Workers check revisions before/after rendering and metadata loading. The UI rejects an incorrect revision or obsolete generation before uploading pixels. Metadata-based revisions cannot detect byte changes that deliberately preserve size and timestamps.

| Resource | Current policy |
| --- | --- |
| Page textures | Strict 160 MiB estimated RGBA payload; LRU updated on use; eviction releases active aliases |
| Thumbnail textures | 64 entries, each at most 192 pixels on the longest edge; oversized cached PNGs are regenerated |
| Disk PNG cache | 384 MiB; old PNGs evicted asynchronously; temporary files excluded |
| Document geometry | At most 20,000 pages, read before opening the reader |
| Process output | 2 MiB for normal PDF metadata/link output, 16 MiB for complete page-box geometry |

Disk cache insertion uses temporary files and publication by rename. Cache failures fall back to rendering. These policies bound retained payloads and work, not total process RSS, GPU memory, temporary conversion buffers or the memory of Poppler children. Resource-exhaustion and pathological-document behavior are not security-sandbox guarantees.

The Poppler XML backend can report zero-width text bounds for rotated links. Eder excludes those invalid hitboxes. Unrotated index links, crop-box/media-box alignment, and rotated page dimensions are tested; complete rotated-link support requires a backend improvement.

## Persistence and shutdown

Existing data stays in the `com / Folio / Folio` application directory, with `folio.sqlite3` and native path encoding preserved. Preferences stay in SQLite. Favorites, MRU and existing reading positions migrate in place. The storage worker holds an OS file lock for the data directory, visibly refusing a second instance; the lock is released automatically after process exit/crash.

Schema version 2 is applied transactionally. It retains legacy row-stride fractions under anchor version 0 and adds versioned content anchors, zoom and normalized horizontal pan. New anchors use version 1. The first real reader viewport converts a legacy anchor using the former fixed-row geometry; subsequent successful saves write content coordinates. New versions of the schema are refused rather than downgraded.

Normal close freezes actions, cancels speculative work and imports, drains accepted storage commands, waits for imports to finish cleanup, and commits the final position together with recovery-target removal. The UI then signals the supervisor to stop, wake and join every worker off the frame thread. Only after those joins does it receive the final close acknowledgment, so Poppler children are reaped before normal application exit.

A failed final transaction rolls back both changes and keeps the window open for retry. A close taking more than five seconds offers an explicit exit without waiting for saves. Worker panics become visible fatal events. No persistence or process cleanup guarantee applies to an OS forced kill.

## Validation

The refactor adds tests for obsolete opens/listings/rasters, revision mismatches, mixed page geometry, horizontal-position saving, cache alias eviction, oversized cached images, import confinement/cancellation, migration compatibility, transaction rollback, exclusive instance locks, queue saturation, full result queues and joined shutdown.

Use:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked -- --include-ignored
cargo build --locked --release
cargo run --locked --release --example profile
```

Poppler-dependent tests require its installed executables. The profile example measures scan/sort, geometry preparation, viewport lookup and cold/warm raster paths. See [architecture validation](architecture-validation.md) for this implementation's actual checks and measurements. A native desktop smoke pass remains necessary before release.
