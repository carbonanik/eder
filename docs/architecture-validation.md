# Architecture refactor validation — 20 October 2026

The application was refactored from a 2,941-line `src/app.rs` into the modules described in [architecture.md](architecture.md). `src/desktop.rs` is now a small eframe/composition adapter, with dispatch and result processing in `src/desktop/`. Existing compatibility APIs and the Folio data directory remain available.

## Completed automated checks

- `cargo fmt --check`.
- `cargo clippy --locked --all-targets -- -D warnings`.
- `cargo test --locked -- --include-ignored`: **34 passed**, including actual Poppler rendering, geometry, internal-link and cache checks.
- `cargo build --locked --release`.
- `cargo run --locked --release --example profile`.

Tests exercise application transitions without a GUI; compact/wide headless egui layouts; visible-page scheduling; viewport anchor conversion; stale raster/session/revision rejection; strict texture eviction including aliases; migration from old databases; future schema refusal; transactional final-save rollback; OS lock exclusion; non-overwriting/cancellable imports; saturated command/results queues; and acknowledgment only after workers have joined and released the instance lock.

The mixed-page PDF integration fixture includes portrait, landscape, intrinsic rotation and a crop box different from the media box. Its geometry is checked against real raster dimensions. Crop-box text links use media-box coordinates. The test also preserves the backend limitation that rotated zero-width text-link bounds do not create an interactive hitbox. Oversized but otherwise valid cached PNGs cannot bypass thumbnail dimensions.

## Release-mode smoke measurements

Both runs used the supplied three-page PDF, temporary caches and a generated directory of 10,000 entries. The baseline was built from a temporary snapshot of the source taken before this refactor. These are individual runs, not a statistically meaningful speed comparison.

| Operation | Before refactor | After refactor |
| --- | ---: | ---: |
| Scan and sort 10,000 entries | 54.80 ms | 41.57 ms |
| PDF page count | 20.89 ms | 20.67 ms |
| Cold thumbnail | 41.75 ms | 41.02 ms |
| Cached thumbnail | 0.174 ms | 0.121 ms |
| 2,400-pixel page raster | 301.05 ms | 244.21 ms |
| Complete page-box geometry, three pages | Not previously measured | 20.72 ms |
| Prepare 20,000-page geometry layout | Not previously available | 0.080 ms |
| 100,000 visible-range lookups | Not previously available | 6.20 ms total |

The synthetic 20,000-page layout repeats fixture dimensions; it does not measure opening a real 20,000-page PDF. Raster dimensions were 1,746 × 2,400 (16,761,600 bytes before upload). The geometry metadata command adds work during document opening; it avoids layout dimensions changing as individual page images arrive.

## Remaining release acceptance

A full native graphical user journey was not performed during this refactor. Headless egui testing does not verify actual GPU presentation, OS file dialogs, scrolling feel, accessibility, desktop frame percentiles, or all Linux/macOS/Windows behavior.

Before distribution, use an isolated application-data directory and a representative corpus to verify:

1. Choose a folder; browse, refresh, filter and import documents at compact and wide widths.
2. Read a mixed-size PDF in both modes; scroll rapidly, jump, use index links, zoom, pan and resize. Reopen it and check the normalized position and zoom.
3. Change favorites/history/settings; restart and verify persistence without changes to original PDFs.
4. Attempt a second instance, force-kill/recover a session, and close during active rendering/imports.
5. Profile p50/p95/p99 frame work, upload duration, RSS/VRAM and cold opening latency on real scanned/vector/large PDFs. The plan's 16.7 ms frame target has not been established by these smoke measurements.

Current explicit limits include 20,000 pages per document, bounded geometry output, a 2,400-pixel raster edge and metadata-based file revisions. Some rotated links remain unavailable because of invalid Poppler bounds. Directory enumeration can delay queued storage operations on a slow filesystem; it stays off the UI thread. These limits are documented behavior, not claims of arbitrary-document performance.
