# Software Requirements Specification

## PDF Library and Reader

**Version:** 1.0 | **Status:** Baseline derived from the current application behavior

## 1. Introduction

### 1.1 Purpose

This document specifies the functional and quality requirements for a personal application used to browse a local collection of PDF documents and read them. It is intended for product, design, implementation, and testing stakeholders. It describes observable product behavior without prescribing implementation technologies or software architecture.

### 1.2 Scope

The product lets a user choose a local folder as a PDF library, browse its immediate subfolders and PDF files, open and read PDFs, resume a document at its previously saved page, and maintain favorites and a currently-reading list.

The product is a local reader and library browser. It does not require an account or a network service.

### 1.3 Intended users

- **Reader:** Selects a library, browses and opens PDF documents, and manages reading preferences.
- **Tester or maintainer:** Verifies the requirements and supported behaviors described in this specification.

### 1.4 Definitions

- **Library root:** The user-selected folder from which browsing begins.
- **Current folder:** The folder currently displayed within the library.
- **Currently Reading:** A list of PDFs the user has opened, ordered with the most recently opened first. It is a reading-history list, not a completion or progress status.
- **Favorite:** A PDF explicitly marked by the user for quick access.
- **Reading position:** The page number last viewed in a particular PDF.

## 2. Overall description

### 2.1 Product perspective

The product provides a visual library browser and an in-app PDF reading experience. It reads PDFs from user-selected local storage and retains user preferences and library lists locally between sessions.

### 2.2 Operating context

The product is intended to run in desktop and compact-screen environments. It shall adapt library navigation and layout to the available screen size. Folder selection depends on the environment providing a supported local folder-selection capability.

### 2.3 Constraints and assumptions

- The user supplies and maintains the PDF files and folder structure.
- A configured library is a directory on storage accessible to the application.
- The library browser lists only immediate child folders and PDF files; it does not recursively flatten the folder tree.
- File and folder names are displayed as provided by the file system.
- Favorites, reading history, reading positions, preferences, and the configured library location are user-specific local data.
- A file may be unavailable after it has been moved, renamed, deleted, or become inaccessible.

## 3. Functional requirements

Requirements are identified as **FR-nn**. “Shall” denotes a required behavior.

### 3.1 Application entry and library configuration

#### FR-01 — Open the library
The product shall provide a Library view as its primary screen.

#### FR-02 — Unconfigured library state
If no library root is configured, the product shall explain that no library folder is configured and direct the user to Settings to choose one.

#### FR-03 — Configure library root
The user shall be able to choose a local folder as the library root in environments that support folder selection.

#### FR-04 — Persist library root
The product shall retain the selected library root between application sessions.

#### FR-05 — Apply changed library root
After the user changes the library root in Settings and returns to the library, the product shall refresh the displayed library from the configured location.

#### FR-06 — Unsupported folder selection
Where folder selection is unavailable, the product shall indicate that the capability is unsupported and shall not present folder selection as successful.

#### FR-07 — Missing or inaccessible root
If the configured root no longer exists or cannot be accessed, the product shall not display it as a successfully loaded library. The user shall be able to configure another root in Settings.

### 3.2 Library browsing and navigation

#### FR-08 — List folder contents
For the current folder, the product shall list its immediate child directories and immediate files whose extension is `.pdf`, without regard to extension letter case. Other file types shall not appear in the library listing.

#### FR-09 — Order library entries
The product shall display folders before PDF files. It shall sort folders and PDFs separately by name, without regard to letter case.

#### FR-10 — Display folder entries
Each folder entry shall show the folder name and a folder visual. Selecting it shall open that folder and display its immediate contents.

#### FR-11 — Display PDF entries
Each PDF entry shall show its file name and a preview of its first page when a preview can be generated. If a preview is unavailable, the product shall display a generic PDF visual instead.

#### FR-12 — Preview caching
The product shall cache generated PDF previews for reuse. A preview-generation or cache-read failure shall not prevent the user from opening the PDF.

#### FR-13 — Navigate to parent folder
When browsing below the library root, the user shall be able to return to the parent folder.

#### FR-14 — Navigate to library root
The user shall be able to return directly to the library root from a nested folder.

#### FR-15 — Keep navigation within the library
Parent navigation shall not take the user above the configured library root.

#### FR-16 — Empty folder state
If a folder contains no immediate subfolders or PDFs, the product shall show an empty-state message explaining that no PDFs or subfolders were found.

#### FR-17 — Refresh on folder navigation
When the user opens a folder or navigates to another folder, the product shall load and display that folder’s contents and update the navigation title.

### 3.3 Library views and quick access

#### FR-18 — Provide primary views
The product shall provide distinct Library, Currently Reading, and Favorites views.

#### FR-19 — Show current view identity
The product shall identify the active view in its navigation and page title.

#### FR-20 — Show navigation counts
Where navigation counts are displayed, the Library count shall represent the immediate folders plus PDFs in the current folder, the Currently Reading count shall represent its stored entries, and the Favorites count shall represent its stored entries.

#### FR-21 — Currently Reading ordering
When a PDF is opened, the product shall add it to Currently Reading. If it is already present, the product shall move it to the top rather than create a duplicate.

#### FR-22 — Currently Reading contents
The Currently Reading view shall display available PDFs from the stored list, most recently opened first.

#### FR-23 — Remove reading-history entry
The user shall be able to remove a PDF from Currently Reading without deleting or modifying the PDF itself.

#### FR-24 — Favorites ordering
The Favorites view shall display available favorited PDFs sorted by file name, without regard to letter case.

#### FR-25 — Favorite or unfavorite a PDF
The user shall be able to add a PDF to Favorites or remove it from Favorites from its library or Favorites entry.

#### FR-26 — Persist quick-access lists
The product shall retain Favorites and Currently Reading between application sessions.

#### FR-27 — Handle unavailable list entries
The product shall not display a Favorite or Currently Reading entry as an available PDF if its file is missing. The product shall retain the stored list entry unless the user explicitly removes it.

#### FR-28 — Empty quick-access states
When Favorites is empty, the product shall explain how to add a favorite. When Currently Reading is empty, the product shall explain that opened PDFs appear there.

### 3.4 PDF reading

#### FR-29 — Open a PDF
Selecting a PDF entry in the Library, Currently Reading, or Favorites view shall open that PDF in the product’s reader.

#### FR-30 — Display PDF pages
The reader shall render the selected PDF’s pages and provide a means to navigate the document using the reader’s document interaction area.

#### FR-31 — Identify the document
The reader shall display the PDF file name.

#### FR-32 — Show reading progress
When the document’s page count is available, the reader shall display the current page number and total page count.

#### FR-33 — Save position per PDF
The product shall retain a reading position separately for each PDF, associated with that file’s location.

#### FR-34 — Resume position
When a PDF with a saved reading position is opened again, the reader shall begin at that position. If no position has been saved, it shall begin at the first page.

#### FR-35 — Save position during reading
The product shall save the current page as the user navigates through a PDF and when the reader closes, so that the position can be restored later.

#### FR-36 — Save position on request
The reader shall provide an explicit Save Position action. After the action completes, the product shall confirm that the position was saved.

#### FR-37 — Minimum valid starting page
The reader shall not attempt to start at a page number below the first page.

#### FR-38 — Toggle reader dark mode
The reader shall provide a user-controlled dark-mode toggle that changes the document viewing appearance.

#### FR-39 — Persist reader dark mode
The product shall retain the reader dark-mode preference between application sessions and use it when opening PDFs.

#### FR-40 — Return from reader
The reader shall provide a Back action that returns the user to the previous product view.

### 3.5 Interrupted-session recovery

#### FR-41 — Restore an interrupted open
If the product has a pending last-opened PDF from an interrupted session, it shall open that PDF when the library is next entered, provided the file remains available.

#### FR-42 — Clear invalid recovery target
If the pending PDF is unavailable, the product shall clear the recovery target and continue without attempting to open the missing file.

#### FR-43 — Clear completed recovery target
After the pending PDF has been handled, the product shall clear the recovery target so that a completed prior reading session is not repeatedly reopened.

### 3.6 Settings and navigation controls

#### FR-44 — Open Settings
The user shall be able to open Settings from the Library view.

#### FR-45 — Display library setting
Settings shall show the currently configured library folder, or clearly indicate that no folder is selected.

#### FR-46 — Confirm folder change
After a library folder is successfully changed, the product shall confirm the change to the user.

#### FR-47 — Return from Settings
Settings shall provide a Back action that returns the user to the Library view.

#### FR-48 — Adapt navigation to screen size
The product shall provide navigation appropriate to available screen width: compact navigation for narrow layouts and persistent view navigation for wider layouts.

#### FR-49 — Provide folder navigation controls
The product shall make parent-folder and library-root actions available when applicable, and shall not offer parent navigation at the library root.

#### FR-50 — Desktop window controls
In a desktop windowed environment, the product shall provide window minimize, maximize/restore, and close actions. The title-bar area shall support moving the window.

## 4. External interface requirements

### 4.1 User interface

- The Library interface shall distinguish folders from PDFs and provide selectable entries.
- PDF entries shall expose a Favorite action. Entries in Currently Reading shall expose a remove-from-list action.
- The reader interface shall expose the document name, page progress when available, dark-mode toggle, Save Position action, and Back action.
- Settings shall expose the library-folder setting and its current value or unconfigured state.
- Interactive actions shall have clear labels or accessible descriptions and provide visible feedback for important completed actions.
- Long file and folder names shall remain identifiable without disrupting the layout.

### 4.2 Local file access

- The product shall read PDF files from the configured library.
- The product shall not alter or delete library files as a side effect of browsing, favoriting, or removing an item from Currently Reading.
- If a selected PDF is no longer available or cannot be read, the product shall communicate that the document could not be opened and preserve the user’s ability to return to the library.

## 5. Data requirements

The product shall persist the following user data locally:

| Data | Required behavior |
|---|---|
| Library root location | Retain between sessions; use to initialize library browsing. |
| Favorites | Store unique PDF locations; support adding and removing entries. |
| Currently Reading | Store unique PDF locations in most-recently-opened order; support removing entries. |
| Reading position | Store the last page number separately for each PDF location. |
| Reader dark-mode preference | Store and reuse across reader sessions. |
| Pending recovery target | Temporarily store the PDF location to restore after an interrupted session; clear after handling. |
| PDF previews | Cache generated previews for reuse; previews are derived data and may be regenerated. |

The product shall treat unavailable file locations as unavailable content, not as a reason to modify or delete the source files.

## 6. Non-functional requirements

### 6.1 Usability and accessibility

**NFR-01** The product shall provide consistent names and actions for Library, Currently Reading, Favorites, Settings, Back, and library-root navigation.

**NFR-02** The product shall provide an understandable visual state when a library is unconfigured, a folder is empty, a quick-access view is empty, a preview is unavailable, or folder selection is unsupported.

**NFR-03** The product shall keep primary browsing and reader actions usable on both compact and wide screens.

### 6.2 Reliability and data integrity

**NFR-04** A failure to generate a PDF preview shall not corrupt the PDF or block access to the document.

**NFR-05** A missing file shall not be represented as successfully opened content.

**NFR-06** Saving or restoring one PDF’s reading position shall not overwrite another PDF’s position.

**NFR-07** User actions that affect persistent state shall report success or failure rather than silently presenting an incorrect result.

### 6.3 Performance

**NFR-08** The product should show a loading indication while library contents or a document are being loaded.

**NFR-09** Reusable previews should be served from cache where available to avoid repeatedly rendering the same preview.

**NFR-10** Opening and navigating large PDFs shall not cause unbounded growth in retained preview or page-image memory.

Specific response-time and maximum-document-size targets are not defined by the current product baseline and shall be set through product acceptance criteria if required.

### 6.4 Privacy and connectivity

**NFR-11** Library contents and personal reading lists shall remain local to the user’s environment; the product shall not require an account or remote service for its stated core functions.

**NFR-12** The product shall not transmit document contents or reading activity to a remote service as part of browsing or reading.

## 7. Out of scope

The following capabilities are not part of the current product requirements:

- Full-text search, filtering by arbitrary query, or user-selectable sorting.
- Recursive indexing of all subfolders into a single flattened listing.
- Importing, copying, moving, renaming, or deleting PDF files.
- PDF annotation, highlighting, comments, editing, or form filling.
- Printing, exporting, or sharing documents.
- Cloud synchronization, accounts, or cross-device synchronization.
- Managing categories independently of the user’s folder structure.
- Tracking reading completion, percentage read, or per-document bookmarks beyond the saved current page.
- User-configurable themes beyond the reader’s dark-mode toggle.

## 8. Acceptance criteria

The product is acceptable when the following end-to-end checks pass:

1. With no configured root, the Library view explains that configuration is needed; selecting a folder in Settings makes its immediate folders and PDFs available.
2. A folder containing mixed-case PDF extensions, non-PDF files, and subfolders displays only the immediate subfolders and PDFs, with folders first and each type alphabetically sorted without regard to case.
3. Opening a nested folder, returning to its parent, and returning to the library root all display the correct contents and title.
4. Opening a PDF adds it to the top of Currently Reading; reopening it does not create a duplicate.
5. Adding and removing Favorites updates the Favorites view and persists after restarting the product.
6. Removing an item from Currently Reading does not remove or modify its source PDF or its Favorite status.
7. Reopening a PDF restores its saved page; the reader displays current and total pages when available, and the explicit save action confirms completion.
8. Changing reader dark mode affects document viewing and persists after restarting the product.
9. Empty folders, empty quick-access views, unavailable previews, unsupported folder selection, and missing files are handled with appropriate user-facing states.
10. A pending recovery target is opened on the next start only if its file remains available, and is cleared after handling.
11. Library navigation and entry layout remain usable in both compact and wide screen configurations.
