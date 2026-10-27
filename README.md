# Eder — native Rust PDF library

A local desktop PDF browser and reader implemented from the supplied PDF Library and Reader SRS. Native egui/OpenGL interface, Rust application logic, SQLite persistence, and Poppler PDF rendering. No webview, account, telemetry, or document upload. Source documents are opened read-only by the renderer.

## Start on Linux

A Linux x86-64 release executable is included in `bin/eder` in the delivery archive. It was compiled on this environment; compatibility with older Linux distributions is not guaranteed. Install Poppler and the normal desktop graphics libraries, then run:

```sh
# Debian / Ubuntu
sudo apt-get install poppler-utils libxkbcommon0 libgl1
chmod +x bin/eder
./bin/eder
```

A desktop X11 or Wayland session with OpenGL support is required. On other architectures or incompatible Linux distributions, build from source below. Choose **Settings → Choose folder**, or enter an absolute folder path. Poppler is a separate runtime dependency and is not bundled.

## Build from source

Install the current stable Rust toolchain with Cargo (the locked Linux dependencies declare a minimum of Rust 1.89). Use the included Cargo.lock.

```sh
# Linux build dependencies (Debian / Ubuntu)
sudo apt-get install build-essential pkg-config libxkbcommon-dev libwayland-dev libgl1-mesa-dev poppler-utils
cargo build --locked --release -j 2
./target/release/eder
```

## Build an AppImage

Install the Linux build dependencies above and `appimagetool`, then run:

```sh
./packaging/appimage/build.sh
```

The AppImage is written to `target/appimage/`. Poppler and the host's desktop graphics libraries remain runtime dependencies; Poppler is not bundled.

Pushes to `master` and manual runs of the **Release AppImage** GitHub Actions
workflow check the package version in `Cargo.toml`. If a release for `v<version>`
already exists, the workflow skips the build. Otherwise, it builds a Linux x86-64
AppImage and publishes it in a GitHub release with generated notes, creating the
version tag at the triggering commit. To publish a new version, update the
package version in `Cargo.toml` and `Cargo.lock`, then push to `master`.
The workflow uses the built-in `GITHUB_TOKEN`; no extra secret is required.

macOS: install Xcode command-line tools, Rust, and Poppler (`brew install poppler`), then `cargo run --locked --release`.

Windows: install Rust's MSVC toolchain and Visual Studio C++ build tools. Install a trusted Poppler build and put its `bin` directory on PATH, or set `EDER_POPPLER_DIR` to that directory (`FOLIO_POPPLER_DIR` remains supported for compatibility). Run `cargo build --locked --release`; the program is `target\release\eder.exe`.

The Rust source uses cross-platform APIs. Only Linux compilation and headless tests were performed here; Windows/macOS packages and native desktop interaction are not yet validated. This is a desktop application, not an Android/iOS build. Compact desktop windows are supported.

## Linux desktop installation and Flathub preparation

After building, install the executable, desktop entry, icon, metadata, and license
notices into a staging directory:

```sh
DESTDIR="$PWD/target/linux-stage" PREFIX=/usr sh packaging/linux/install.sh
sh packaging/linux/validate.sh
```

The desktop application ID is `io.github.carbonanik.eder`. See
[Flathub readiness](docs/flathub-readiness.md) for remaining release, packaging,
sandbox testing, and disclosure work. This repository does not yet contain a
Flathub manifest or a tested Flatpak build.

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

## Using Eder

1. On first launch, open **Settings → Choose folder…** and select the folder that contains your PDFs.
2. Browse the folder's immediate subfolders and PDFs. Open a folder to navigate into it; use the parent and library-root controls to move back. Use **Refresh** to reload the current folder after changing files outside Eder.
3. Open a PDF to read it. Eder remembers each PDF’s page, within-page offset, zoom and horizontal pan. Use **Save Position** to explicitly save and confirm the current page.
4. Mark a PDF as a favorite for quick access. Open **Currently Reading** for recently opened PDFs; removing an entry from that list does not delete or modify the source file.

Choose **Settings → Reading Mode** to switch between **Single Page** and **Continuous Scroll**; the selection is saved locally. In continuous mode, visible pages load incrementally and up to three pages before and after the visible range are prefetched to reduce blank-page flicker while scrolling. Press **F1** or click **Shortcuts** for the keyboard reference. Press **F11** (also **Control+Command+F** on macOS) to toggle fullscreen. The reader toolbar is hidden in fullscreen; keyboard shortcuts remain available. Escape dismisses shortcut help, exits fullscreen, leaves a focused field, or returns from the reader to the library, in that order. Use scrollbars or a trackpad to pan a zoomed page. Dark mode inverts document colors, including images. Text selection, search, annotations, printing, and password entry are not implemented. Password-protected PDFs that require a password produce an error.

Keyboard shortcuts use **Ctrl** on Linux/Windows and **Command** on macOS:

| Shortcut | Action |
| --- | --- |
| Ctrl/Command+O | Add PDFs from the library |
| Ctrl/Command+K or +F | Search library documents |
| F5 or Ctrl/Command+R | Refresh library documents |
| Ctrl/Command+1, +2, +3 | Library, Currently Reading, Favorites |
| Ctrl/Command+, | Open Settings from the library |
| Left/Right, Page Up/Page Down | Previous/next page |
| Up / Down | Scroll up/down (hold to keep scrolling) |
| Shift+Space / Space | Previous/next page |
| Home / End | First/last page |
| Ctrl/Command+S | Save reading position |
| Ctrl/Command++ / +- | Zoom in/out (Ctrl/Command+= also zooms in) |
| Ctrl/Command+0 | Reset zoom |
| Ctrl/Command+W | Close the document, or close the library window |
| Ctrl/Command+Q | Quit and save pending changes |

Page navigation shortcuts are inactive while editing a text field. Library search searches document names; PDF text search is not implemented.

## Performance design

| Resource | Policy |
| --- | --- |
| Library UI | Virtualized rows; only visible rows produce widgets and request previews |
| Page cache | LRU texture handles; 160 MiB of retained uncompressed RGBA page data |
| Preview cache in memory | At most 64 entries, each at most 192 × 192 pixels (~9 MiB maximum pixel payload) |
| Page and preview cache on disk | 384 MiB budget, oldest writes evicted; transient temporary files may exist during insertion |
| Page raster | Longest edge capped at 2,400 pixels; no full-document rasterization |
| Pending page work | One replaceable latest-request slot; obsolete subprocesses cancelled |
| Pending previews | 12 queued requests, one worker, four results maximum |
| Page results | One bounded result slot |
| Persistence and directory reads | Dedicated worker; UI does not wait for SQLite or directory enumeration |
| Idle rendering | Event-driven repaints; no continuous animation loop |
| PDF execution | Separate page, metadata, link and preview lanes; at most two active PDF operations; subprocess timeout 30 seconds |
| Release build | Standard release optimization, one codegen unit, stripped binary |

These are cache and pipeline limits, **not a hard total-process RSS guarantee**. GPU copies, temporary image conversion buffers, fonts, GUI state, and Poppler parsing require additional memory. Directory metadata and saved lists scale with entry count. Poppler may consume substantial memory for pathological PDFs. It is process-isolated for responsiveness and failure containment, not a security sandbox. Do not claim unmeasured performance for arbitrary large documents.

Poppler starts a subprocess on each cold page render. Recent-page and preview reuse avoids that cost, but a persistent PDF engine may be appropriate if measurements on your actual documents show cold-page latency is unacceptable. Raster resolution adapts in coarse buckets up to the 2,400-pixel cap; zoom beyond that cap uses GPU scaling.

## Data and recovery

Data continues using the existing OS application-data directory returned by `directories::ProjectDirs` for `com / Folio / Folio`, so upgrading preserves saved library settings and reading progress. The database is `folio.sqlite3`; cached previews use its separate cache directory under `previews-v1`.

- SQLite WAL and FULL synchronous writes protect completed transactions.
- Position updates are debounced for 650 ms; document changes, Back, explicit Save and normal close submit the latest position. Normal close commits its final position and recovery cleanup atomically, then joins workers before exiting. A forced kill can lose an update still in the queue.
- Source paths are canonicalized; native path bytes are stored without lossy UTF-8 conversion.
- A pending target is stored after a successful document metadata open. It is consumed and cleared transactionally at startup. Missing targets are cleared. A recovered session does not immediately re-arm the target, preventing repeated recovery loops.
- Refresh reloads folder contents and invalidates in-memory previews. Disk previews are keyed by path, file size, and nanosecond modification time. Deliberately replacing bytes while preserving both size and timestamp requires clearing the preview cache.
- One instance per data directory is enforced by an OS file lock; a second instance reports an error.
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

`profile` measures a 10,000-entry temporary directory, document metadata, a cold thumbnail, cached thumbnail, and a full page. It creates and removes its own temporary data. Results depend on the machine, filesystem, and PDF. See [architecture validation](docs/architecture-validation.md) for current checks and measurements. `VALIDATION.md` records the earlier delivery baseline.

## Architecture and source map

The implemented architecture and module boundaries are documented in [docs/architecture.md](docs/architecture.md), with the original migration plan in [docs/architecture-plan.md](docs/architecture-plan.md).

- `src/desktop.rs`, `src/desktop/`: eframe adapter and bounded frame orchestration.
- `src/app/`: headless controller, actions/events, reader geometry and scheduling.
- `src/domain/`: document identity, render keys, positions, settings and errors.
- `src/ui/`: feature widgets, presentation state, theme and texture LRU.
- `src/services/`: use cases and PDF/filesystem/repository interfaces.
- `src/runtime/`: supervised workers, bounded transport, cancellation and shutdown.
- `src/infrastructure/`: Poppler, SQLite migrations, filesystem, disk cache and platform adapters.
- `src/policy.rs`: shared resource limits and resolution buckets.
- `src/library.rs`, `src/render.rs`, `src/store.rs`: compatibility exports.
- `tests/rendering.rs`: real renderer, mixed-size/rotated/cropped page and cache-failure integration tests.
- `examples/profile.rs`: repeatable performance smoke measurements.

Current limits include 20,000 pages per document. Some rotated text links cannot be made interactive because Poppler reports invalid bounds; Eder discards those hitboxes. Native graphics and frame latency need a desktop acceptance pass; see [docs/architecture-validation.md](docs/architecture-validation.md).

MIT license applies to this application's source. Dependencies retain their own licenses. Poppler is separately installed; consult its license and dependency licenses before redistributing bundled binaries.
