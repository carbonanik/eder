# Validation — 16 October 2026

Environment: Linux x86-64, Ubuntu 24.04, Rust 1.99.1. Dependencies pinned by Cargo.lock. These results are for this build environment, not performance guarantees for other computers or documents.

## Checks completed

- `cargo check`: passed.
- `cargo build --locked --release -j 2`: passed; Linux x86-64 executable included. Standard release optimization is used because thin LTO produced unresolved-symbol errors with this toolchain.
- Dynamic-library resolution (`ldd`): no missing linked libraries in the build environment.
- `cargo fmt --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test --locked -- --include-ignored`: **7 passed**, zero failures.
- Renderer tests used real `pdfinfo` and `pdftoppm`, not mocks.

The tests cover immediate-child scanning, mixed-case PDF extensions, ordering, boundary/symlink escape prevention, unique MRU ordering, durable settings and favorites, independent/clamped positions, history removal without file deletion, missing-file retention, one-shot recovery, bounded disk cache eviction, cancellation, corrupt PDFs, failed cache writes, corrupt cache regeneration, real page dimensions, cached thumbnail reuse, virtualized 10,000-item layouts at compact/wide sizes, and page navigation clamping.

## Release-mode smoke measurements

Command: `cargo run --locked --release --example profile`.

| Operation | Observed time |
| --- | ---: |
| Scan and sort 10,000 immediate PDF entries | 32.99 ms |
| Read metadata of supplied three-page PDF | 21.10 ms |
| Generate first-page preview, cold cache | 21.51 ms |
| Read same preview from cache | 0.073 ms |
| Render one page at a 2,400-pixel longest edge | 238.63 ms |

Rendered page: 1,746 × 2,400 pixels; 16,761,600 bytes of RGBA pixels before GPU upload. The supplied PDF is a small synthetic text document. One run, no statistical confidence interval, no representative scanned/complex/large-document corpus. The environment's Poppler executable may include a runtime wrapper, so subprocess launch cost may differ on a normal desktop.

## Not verified here

- Native desktop display, file portal, OS titlebar interaction, GPU presentation, and a complete manual user journey. No graphical display server is available in the environment. The egui layout test is headless and does not replace visual review.
- Windows and macOS builds or installers. The included binary targets Linux x86-64.
- Total application RSS/VRAM, battery usage, cold startup, or latency percentiles on representative large documents.
- Full keyboard and assistive-technology audit.
- Multiple simultaneous app instances, installer signing, store distribution, or automatic updates.

## Desktop acceptance pass before distributing

1. Start with clean application data, choose a folder in Settings, and check nested navigation and native window controls.
2. Open a PDF, change page, save, close normally, restart, and reopen; verify position and dark preference.
3. Favorite and remove history entries; verify source PDFs remain byte-identical.
4. Rename a favorite outside the app; verify its unavailable state and explicit removal.
5. Kill the process while reading, restart, verify one recovery, then restart again after a completed session.
6. Use 360px and 1100px windows; inspect long filenames, toolbar wrapping, zoom, and folder picker behavior.
7. Profile actual large scanned/vector PDFs and agree measurable acceptance targets before describing the app as production-performance validated.
