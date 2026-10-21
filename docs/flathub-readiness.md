# Flathub readiness

Eder has upstream desktop integration for `io.github.carbonanik.eder`. It is not
yet ready for submission. This is an implementation checklist, not submission
PR text. No Flatpak manifest or submission interaction was generated.

## Upstream files

- `packaging/linux/io.github.carbonanik.eder.desktop`: launcher and discovery keywords.
- `packaging/linux/io.github.carbonanik.eder.metainfo.xml`: AppStream description,
  project links, age rating, screenshot reference, and existing tagged release.
- `packaging/linux/install.sh`: stages a built executable, desktop metadata,
  the existing SVG icon, application license, and checked-in dependency notices.
- `packaging/linux/validate.sh`: validates desktop and AppStream metadata locally.
  Pass `--flathub` to also run the installed Flathub metadata linter.
- `src/main.rs`: window application ID matches the desktop filename.

The installer accepts `PREFIX` (default `/usr/local`), `DESTDIR` (default empty),
and an optional first argument naming the executable. It does not build or
install Poppler. Invoke it with `sh`; root privileges are unnecessary for staging.

```sh
cargo build --locked --release
DESTDIR="$PWD/target/linux-stage" PREFIX=/usr sh packaging/linux/install.sh
sh packaging/linux/validate.sh
sh packaging/linux/validate.sh --flathub
```

Existing native application data continues to use the Folio paths for upgrade
compatibility. Flatpak supplies its own XDG directories, so native installation
data does not automatically migrate into a Flatpak installation.

## Work still required

1. Make the source, release, homepage, and screenshot publicly reachable. The
   pinned screenshot URL returned HTTP 404 on 2026-10-21. Metadata currently
   references the existing screenshot at commit
   `132fa634b47173b22e4de1242e23285e2943bf32`; replace it with a public, immutable
   URL if that commit will not be published. Confirm the image reflects the
   release and that displayed document covers can be redistributed. Prefer new
   screenshots using documents you own or openly licensed sample documents.
2. Publish a stable tagged release containing the upstream desktop integration.
   Cargo currently declares 0.1.1, while the only local release tag is v0.1.0.
   AppStream records the existing 0.1.0 tag dated 2026-10-18; add the actual next
   release and date when it is published. Do not invent a release date.
3. Have a human independently author the Flathub manifest. Flathub prohibits
   AI-generated and AI-assisted manifests, including disclosed ones. Do not copy
   AI output into a manifest or ask an agent to revise it. Consult the official
   documentation and maintained packaging examples directly.
4. Provide a complete offline source build, including locked Cargo dependencies
   and Poppler. Core functionality uses `pdfinfo`, `pdftoppm`, and `pdftohtml`;
   relying on host-installed commands is insufficient. Install the licenses for
   bundled modules and their dependencies. Recheck the checked-in Rust notices
   against the final dependency lockfile and supported architectures.
5. Use the latest supported Flathub runtime at submission time, and minimize
   permissions using supported portals. The app already uses rfd's portal file
   and folder pickers. No application feature requires network access. Verify
   access to selected folders and persistence of grants before deciding whether
   any additional application work is needed.
6. Build, install, and run the actual Flatpak and complete the acceptance checks
   below. Run Flathub's manifest, AppStream, and repository linters; local
   AppStream syntax validation alone does not establish Flathub readiness.
7. Establish development history, evidence of use, and an ongoing maintenance
   commitment. Local commits start on 2026-10-17; Flathub generally rejects very
   new projects without meaningful history, subject to reviewer discretion.
8. Personally disclose known AI-generated material, identifying affected parts
   and approximate extent. A human must write and open the submission PR and
   write its commit messages, description, and review replies. Do not request
   an AI-agent review of the Flathub submission.

## Sandbox acceptance checks

- Start in a fresh sandbox on Wayland and X11; confirm launcher icon and window
  association, graphics, and accessibility behavior.
- Choose a library through the folder portal, browse subfolders, render previews,
  and open PDFs. Test paths with spaces and non-ASCII characters.
- Restart and verify access to the library, saved positions, favorites, and
  currently reading items. Revoking a portal grant should produce a recoverable
  error and allow selection of another folder.
- Import PDFs through the file portal. Confirm the chosen library grants the
  required write access for imports, duplicate names do not overwrite documents,
  and imported files survive relaunch. Existing source PDFs remain unchanged.
- Verify all three Poppler commands run inside the sandbox, including page
  rendering and internal links, with no host Poppler installation.
- Test manual folder entry with an inaccessible path; it should report an error.
  Typing a path does not itself grant sandbox access.
- Test missing, corrupt, and password-protected documents, interrupted reading,
  and the second-instance lock. Check both supported build architectures.

## Validation performed on 2026-10-21

Desktop validation and strict offline AppStream validation passed, including
the staged installed copies. Shell syntax checks, `cargo fmt --check`,
`cargo check --locked --offline`, and all 34 Rust tests (including Poppler
integration tests) passed. The staging install used the existing release
executable; no new release binary was produced.

Flathub's AppStream linter failed with `screenshot-image-not-found`. A separate
HTTP check confirmed the pinned screenshot returned 404. No full Flatpak build,
manifest/repository lint, or sandbox desktop acceptance run was possible without
a human-authored manifest. Those checks remain required.

## AI provenance for this preparation

This preparation used an AI coding agent to create the four files under
`packaging/linux/`, add the application ID in `src/main.rs`, add the desktop
metadata CI job, add the README installation section, and write this document.
The existing SVG is reused without edits by this preparation. This inventory
describes this session only; the maintainer must investigate and account for
any earlier AI-generated code, assets, documentation, and packaging. Human
review does not remove the disclosure requirement.

## Official references

- [Submission](https://docs.flathub.org/docs/for-app-authors/submission)
- [Requirements and generative AI policy](https://docs.flathub.org/docs/for-app-authors/requirements#generative-ai-policy)
- [MetaInfo guidelines](https://docs.flathub.org/docs/for-app-authors/metainfo-guidelines)

Policies were consulted on 2026-10-21. Read the current policy again before
submitting. Do not include this checklist as submission PR prose.
