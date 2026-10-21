# Eder — Copywriting & Messaging Guide

> Project reference for product copy, website content, release announcements, README descriptions, and future marketing materials.
>
> **Status:** Working brand guide · **Updated:** 2026-10-21

## 1. Product identity

**Product:** Eder  
**Category:** Desktop ebook reader and personal bookshelf  
**Core positioning:** **Your personal bookshelf, on your desktop.**  
**Brand promise:** A simple, comfortable place to browse and read the books already on your computer.

Eder is built for people who collect and read digital books. It shows their existing folder structure in the app, lets them read local PDF files, and saves reading progress so they can continue where they left off.

### What Eder is

- A desktop app for a personal collection of local ebooks.
- A PDF reader with zooming, scrolling, and saved reading progress.
- A bookshelf that reflects user-selected folders and their subfolders.
- A reader-first tool, rather than a general document administration system.

### What Eder is not

- An ebook marketplace or download service.
- A cloud library that requires users to upload books.
- A promise to support every ebook format today.
- A file manager replacing the user's existing folder organization.

## 2. Approved primary copy

Use this as the default copy for the landing-page hero and product banner.

**Title**  
Your personal bookshelf, on your desktop.

**Tagline**  
A quieter way to read.

**Short description**  
Eder is a desktop PDF reader designed for people who love collecting and reading books. Browse your existing folders, keep your collection organized, and pick up right where you left off — all without moving your books or uploading them anywhere.

**Supporting feature labels**  
Local books / Familiar folders / Reading progress

### Shorter variations (for limited space)

**One-line description**  
A desktop home for the PDF books you already own, with familiar folders and saved reading progress.

**Repository subtitle**  
A desktop PDF reader and personal bookshelf for your local ebook collection.

**Social / release summary**  
Meet Eder: a desktop reader for your own PDF book collection. Browse your existing folders, read comfortably, and continue where you left off.

These are alternative lengths of the same message—not separate product positions.

## 3. Messaging priorities

In this order:

1. **Your own bookshelf.** Put the user's book collection at the center of the story.
2. **Works with existing folders.** Users select folders and browse their subfolders inside Eder; they do not have to reorganize files for the app.
3. **Read and return.** Zoom, scroll, and resume reading using saved progress.
4. **Local-first experience.** Books stay on the user's computer; do not imply a cloud upload workflow.
5. **Linux motivation.** Eder grew out of a desire for a good desktop ebook reader on Linux. Use this origin story where relevant, without claiming unsupported platform availability.
6. **Implementation details last.** Rust and technical libraries belong in developer-facing content, not the headline.

## 4. Voice and tone

**Be:** calm, clear, reader-focused, practical, personal, confident but modest.

**Avoid:** inflated claims, trendy SaaS jargon, dramatic comparisons, or generic productivity promises.

| Prefer | Avoid |
| --- | --- |
| “Your books, in the folders you know.” | “Revolutionize document management.” |
| “Pick up where you left off.” | “Unlock unparalleled productivity.” |
| “Browse your existing collection.” | “Next-generation digital asset hub.” |
| “Built for reading.” | “The ultimate solution for everyone.” |
| “Desktop PDF reader” | “Universal ebook reader” (until format support grows) |

Write about **the reader's benefit first** and the implementation second. Prefer short, concrete sentences. Use *books*, *reading*, *shelf*, and *folders* more often than *assets*, *documents*, *management*, or *infrastructure*.

## 5. Feature claims: what is safe to say

| Feature or claim | Current understanding | Marketing guidance |
| --- | --- | --- |
| Reading local PDF books | Implemented | Can advertise |
| Browse chosen folders and subfolders | Implemented | Can advertise |
| Folder hierarchy mirrors filesystem | Implemented | Can advertise |
| Manual refresh of library | Implemented | Mention in feature docs when relevant |
| Zoom and scroll while reading | Implemented | Can advertise |
| Saved reading progress | Implemented | Can advertise |
| Bookmarks | In development | Do not describe as available |
| Comments / annotations | In development | Do not describe as available |
| CBR/CBZ comic support | Planned | Do not describe as available |
| Automatic detection of folder changes | Not implemented | Do not claim automatic sync |
| Free and open-source | Long-term commitment undecided | State only what is currently true and verifiable |
| Free forever / no premium features ever | Not promised | **Never promise** |
| Linux support | Linux is the original motivation | Confirm tested release platforms before listing them |

**Release rule:** A planned or partially built feature is not a shipped feature. Move it into public marketing copy only after it is implemented, tested, and available in a released build.

**Accuracy rule:** “Local-first” does not automatically mean the app never accesses a network or gathers any diagnostics. Do not make absolute privacy, offline, or security claims without checking actual behavior.

## 6. Copy by placement

### Website hero / banner

- **Headline:** Your personal bookshelf, on your desktop.
- **Tagline:** A quieter way to read.
- **Description:** Use the approved short description in §2.
- **Feature strip:** Local books / Familiar folders / Reading progress.
- **Visual:** Show the actual Eder interface using a current screenshot. Do not invent UI or imply unfinished features are available.

### GitHub README

Lead with the repository subtitle from §2. Follow with a practical explanation of how to choose folders, browse books, open PDFs, and resume reading. Mention Rust in the technical overview, not as the main product value.

### App listing / release page

Lead with what people can do today. Specify supported operating systems and file formats for that exact release. Make installation requirements and limitations easy to find.

### Feature announcements

Use this structure:

1. **What changed:** name the shipped feature.
2. **Why it helps readers:** give one relatable reading scenario.
3. **How it works:** explain in plain language.
4. **Availability:** state which version includes it.

### About / origin story

Eder began as a personal project inspired by the need for a comfortable desktop ebook reader, particularly on Linux. Its aim is to bring the feeling of a personal bookshelf to books already stored on the computer, without making people change how they organize their collection.

Do not present opinions about competitors as verified universal facts. You may mention ReadEra as inspiration in developer notes, but avoid describing Eder as an official replacement or affiliation.

## 7. Future copy roadmap

These are **draft concepts, not approved live feature claims**.

| When this ships | Possible reader-focused message | Proof to verify before publishing |
| --- | --- | --- |
| Bookmarks | “Save the pages you want to return to.” | Create, view, and navigate bookmarks work in release builds |
| Comments / annotations | “Keep your thoughts beside what you read.” | Exact comment or annotation behavior matches the claim |
| CBR/CBZ reading | “Make room for your comics, too.” | Supported formats are stable and documented |
| Automatic folder watching | “Add a book to your folder. See it on your shelf.” | File changes are detected reliably without manual refresh |
| Expanded platform support | “Your bookshelf, across more desktops.” | Releases and testing exist for each named OS |

Future additions should extend the personal-bookshelf idea—not replace it with a broader “document management” identity unless the product itself changes direction.

## 8. Copywriting checklist

Before publishing any copy, check:

- [ ] Does it make Eder sound like a **reader for books**, not a generic file manager?
- [ ] Does it explain a real benefit in language a non-developer understands?
- [ ] Is every mentioned feature actually released?
- [ ] Are supported formats and platforms accurate for the current build?
- [ ] Have we avoided “free forever,” “no premium ever,” and unverified privacy promises?
- [ ] Have we avoided comparing Eder to a competitor without evidence?
- [ ] Is the length appropriate for the banner, README, app store, or release note?
- [ ] Does the screenshot match the actual shipping interface?
- [ ] Does the copy still fit the central idea: **Your personal bookshelf, on your desktop**?

## 9. Maintaining this guide

Keep this document in the project repository (for example, `docs/EDER_COPYWRITING_GUIDE.md`). When a feature ships:

1. Update the feature-claims table with its verified status.
2. Revise affected short descriptions and feature strips.
3. Update release notes and platform/format support.
4. Preserve the core positioning unless real user research or product direction justifies changing it.
5. Record important decisions below.

### Messaging decision log

- **2026-10-21:** Selected **“Your personal bookshelf, on your desktop.”** as the product identity.
- **2026-10-21:** Kept **“A quieter way to read.”** as the supporting tagline.
- **2026-10-21:** Prioritized existing-folder browsing and saved reading progress in public copy.
- **2026-10-21:** Deferred marketing claims about bookmarks, comments, CBR/CBZ, and automatic folder detection until release.
- **2026-10-21:** Decided **not** to commit in copy to free/open-source status forever or to permanent absence of premium offerings.

---

**Guiding principle:** Sell the reading experience you have built, not the roadmap you hope to build.
