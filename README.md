# Folio — native Rust PDF library

A local desktop PDF browser and reader implemented from the supplied PDF Library and Reader SRS. Native egui/OpenGL interface, Rust application logic, SQLite persistence, and Poppler PDF rendering. No webview, account, telemetry, or document upload. Source documents are opened read-only by the renderer.

## Start on Linux

A Linux x86-64 release executable is included in `bin/folio-reader` in the delivery archive. It was compiled on this environment; compatibility with older Linux distributions is not guaranteed. Install Poppler and the normal desktop graphics libraries, then run:

```sh
# Debian / Ubuntu
sudo apt-get install poppler-utils libxkbcommon0 libgl1
chmod +x bin/folio-reader
./bin/folio-reader
```

A desktop X11 or Wayland session with OpenGL support is required. On other architectures or incompatible Linux distributions, build from source below. Choose **Settings → Choose folder**, or enter an absolute folder path. Poppler is a separate runtime dependency and is not bundled.

## Build from source

Install the current stable Rust toolchain with Cargo (the locked Linux dependencies declare a minimum of Rust 1.89). Use the included Cargo.lock.

```sh
# Linux build dependencies (Debian / Ubuntu)
sudo apt-get install build-essential pkg-config libxkbcommon-dev libwayland-dev libgl1-mesa-dev poppler-utils
cargo build --locked --release -j 2
./target/release/folio-reader
```

macOS: install Xcode command-line tools, Rust, and Poppler (`brew install poppler`), then `cargo run --locked --release`.

Windows: install Rust's MSVC toolchain and Visual Studio C++ build tools. Install a trusted Poppler build and put its `bin` directory on PATH, or set `FOLIO_POPPLER_DIR` to that directory. Run `cargo build --locked --release`; the program is `target\release\folio-reader.exe`.

The Rust source uses cross-platform APIs. Only Linux compilation and headless tests were performed here; Windows/macOS packages and native desktop interaction are not yet validated. This is a desktop application, not an Android/iOS build. Compact desktop windows are supported.

## Features

- Immediate folders and PDFs only; case-insensitive extensions and name sorting, folders first.
- Parent and library-root navigation, with canonical-path boundary checks for symlinks.
- First-page previews generated only for visible rows, disk cached with file metadata invalidation.
- Library, Currently Reading (MRU, no duplicates), and Favorites.
- Missing quick-access items remain stored, visibly unavailable, and removable.
- Single-page and continuous-scroll reading modes, page navigation, page jump, fit-to-window, zoom, and dark-page inversion.
- Per-document automatic position saving and explicit Save Position confirmation.
- Interrupted-session recovery; graceful closes clear the recovery target.
- Native OS window decorations and controls; compact navigation below 760 logical pixels.
- Local SQLite transactions for preferences and reading state; failed writes produce visible errors.

Choose **Settings → Reading Mode** to switch between **Single Page** and **Continuous Scroll**; the selection is saved locally. In continuous mode, visible pages load incrementally as you scroll. Reader keyboard controls: Left/Right or Page Up/Page Down, Home/End, Escape to return. Use scrollbars/trackpad to pan a zoomed page. Text selection, search, annotations, printing, and password entry are not implemented. Password-protected PDFs that require a password produce an error. Dark mode inverts document colors, including images.

## Performance design

| Resource | Policy |
| --- | --- |
| Library UI | Virtualized rows; only visible rows produce widgets and request previews |
| Page cache | LRU texture handles; 48 MiB of retained uncompressed RGBA page data |
| Preview cache in memory | At most 64 entries, each at most 192 × 192 pixels (~9 MiB maximum pixel payload) |
| Preview cache on disk | 128 MiB budget, oldest writes evicted; a transient extra file may exist during insertion |
| Page raster | Longest edge capped at 2,400 pixels; no full-document rasterization |
| Pending page work | One replaceable latest-request slot; obsolete subprocesses cancelled |
| Pending previews | 12 queued requests, one worker, four results maximum |
| Page results | One bounded result slot |
| Persistence and directory reads | Dedicated worker; UI does not wait for SQLite or directory enumeration |
| Idle rendering | Event-driven repaints; no continuous animation loop |
| PDF execution | One page worker and one preview worker; subprocess timeout 30 seconds |
| Release build | Standard release optimization, one codegen unit, stripped binary |

These are cache and pipeline limits, **not a hard total-process RSS guarantee**. GPU copies, temporary image conversion buffers, fonts, GUI state, and Poppler parsing require additional memory. Directory metadata and saved lists scale with entry count. Poppler may consume substantial memory for pathological PDFs. It is process-isolated for responsiveness and failure containment, not a security sandbox. Do not claim unmeasured performance for arbitrary large documents.

Poppler starts a subprocess on each cold page render. Recent-page and preview reuse avoids that cost, but a persistent PDF engine may be appropriate if measurements on your actual documents show cold-page latency is unacceptable. Zoom is GPU scaling of the capped raster; extreme zoom does not increase raster resolution.

## Data and recovery

Data uses the OS application-data directory returned by `directories::ProjectDirs` for `com / Folio / Folio`; the database is `folio.sqlite3`. Cached previews use its separate cache directory under `previews-v1`.

- SQLite WAL and FULL synchronous writes protect completed transactions.
- Page updates are queued on every navigation event and drained before a normal window close. A forced kill can lose an update still in the queue.
- Source paths are canonicalized; native path bytes are stored without lossy UTF-8 conversion.
- A pending target is stored after a successful document metadata open. It is consumed and cleared transactionally at startup. Missing targets are cleared. A recovered session does not immediately re-arm the target, preventing repeated recovery loops.
- Refresh reloads folder contents and invalidates in-memory previews. Disk previews are keyed by path, file size, and nanosecond modification time. Deliberately replacing bytes while preserving both size and timestamp requires clearing the preview cache.
- Run one application instance at a time; simultaneous instances are not coordinated for recovery-target ownership.
- PDFs and their directory structure are never modified by the application.

## Verify

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked -- --include-ignored  # requires Poppler
cargo run --locked --release --example profile
# Optionally profile a real PDF:
cargo run --locked --release --example profile -- /absolute/path/document.pdf
```

`profile` measures a 10,000-entry temporary directory, document metadata, a cold thumbnail, cached thumbnail, and a full page. It creates and removes its own temporary data. Results depend on the machine, filesystem, and PDF. See `VALIDATION.md` for the measurements and limitations of this delivery.

## Source map

- `src/app.rs`: native interface, background workers, cancellation and bounded queues, texture caches.
- `src/library.rs`: immediate directory scanning, sorting, navigation boundaries.
- `src/store.rs`: SQLite schema, settings, favorites, MRU, positions, recovery.
- `src/render.rs`: Poppler subprocesses, timeout/cancellation, bounded raster decode, disk cache.
- `tests/rendering.rs`: real renderer integration tests and cache failure tests.
- `examples/profile.rs`: reproducible performance smoke measurements.
- `.github/workflows/ci.yml`: formatting, lint, tests, release build on Linux.

MIT license applies to this application's source. Dependencies retain their own licenses. Poppler is separately installed; consult its license and dependency licenses before redistributing bundled binaries.
