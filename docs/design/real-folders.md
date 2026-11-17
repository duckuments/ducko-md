# Design draft: switching the library to real directories

Status: Phase 1 (format and migration) is implemented, see docs/FORMAT.md; phases 2-4 are not done. Source: #151, related #112, #125.

## Problem

Folders are currently virtual (see FORMAT.md):

- All content sits flat under `docs/<id>.xml` / `docs/<id>.md`, with the filename being the id, not the title.
- A folder is just a Meta entry in `index.json` (`folder` is the name, `parent` points to the one above), with no corresponding directory on disk.
- Document links are written as `<cite type="doc" doc-id="…">`, relying on "filename = id" to find the target; in Markdown documents and Markdown exports, document links are left as plain title text.

The result: Finder, AI tools, sync drives, and Git can't see the folder structure; when an AI writes files into the library it doesn't know where to put them; and links break entirely once exported elsewhere.

## Principles

1. **Disk is the single source of truth.** The sidebar's folder tree is the directory tree on disk, and the filename is the title: `Work/Weekly Report.md`, `Work/Proposal.xml`. Moving or renaming inside the app moves or renames the real file; changes made outside the app are honored too.
2. **`.xml` and `.md` have equal standing.** Both are named by title and live in the user's own directory. DocxXML is tagged plain text — the user doesn't necessarily read it, it just needs to be readable/writable by AI.
3. **Links are written as relative paths within the file.** They don't depend on the index, so links still work after copying an entire folder elsewhere.
4. **`index.json` is just a rebuildable cache.** Deleting it doesn't lose content, structure, or links — only extras like pinning, starring, and manual ordering.
5. **The user's directory contains only the user's files.** The app's internal data goes into a hidden directory.

## Directory layout (FORMAT 4)

```
Quiet.md/
├── Work/
│   ├── Weekly Report.md
│   ├── Proposal.xml
│   └── Subfolder/…
├── Quick Note.md
└── .quiet/
    ├── format.json      {"format": 4}
    ├── index.json       relative path → extra info (see below)
    ├── settings.json
    ├── .lock
    ├── trash/           recycle bin (TBD, see "Open questions")
    └── thumbs/          file preview thumbnails
```

Scanning rule: recursively walk the library, recognizing `.md` and `.xml`; skip directories and files starting with `.` (`.quiet/`, `.git/`, `.obsidian/`, etc.) and image directories (see "Open questions").

## Index

`index.json` records extra info keyed by relative path: `pinned`, `starred`, `opened`, `order`, `width`, `reading`, `catalogue_closed`, `info`, `refs_hidden`, plus manual ordering for folders. It no longer records `folder`, `parent`, `name`, or `markdown` — these are all determined by the path.

After an external rename or move, paths no longer match:

- On the next scan, pair up "vanished old paths" with "newly appeared paths" using file identity (Unix inode / Windows file id, falling back to content hash), and reattach the extra info to the new path.
- Old entries that can't be paired are discarded, and new files are treated as new documents. Content is never lost — at most, things like pinning are lost.

Each document can still be assigned a runtime-only id in memory, for use by the editor, windows, undo, etc., but it is never written to any file and is never the basis for links.

## Links

- **Markdown:** standard syntax `[Weekly Report](../Work/Weekly Report.md)`, with the path relative to the current file. GitHub, Obsidian, and VS Code can all navigate it. On read, Obsidian's `[[Weekly Report]]` syntax is also supported (looked up by filename in the library); on write, the standard syntax is always used.
- **DocxXML:** `<cite type="doc" href="../Work/Weekly Report.md">Weekly Report</cite>`, also a relative path; `doc-id` is still recognized when reading old files.
- **In-app move/rename:** use the existing backlink data (`Document::doc_links`) to find all referencing documents, rewrite the paths within them, and save. Same idea as Obsidian's "automatically update internal links."
- **External rename:** links may break. On scan, first pair by file identity (as above) and rewrite referencing documents; if unpaired, look for a unique match by filename in the library; if still not found, mark it as a broken link in the body rather than silently deleting it.
- **Export:** exporting a single document rewrites links to be relative to the export location, or falls back to title text when the target isn't included in the export, counting it as an export warning.

## Watching for external changes

- Use filesystem watching (e.g. the `notify` crate, covering macOS FSEvents, Windows, and Linux inotify), coalescing bursts of events in a short window before rescanning the affected directories.
- Rescanning when the window returns to the foreground (existing behavior) is kept as a fallback.
- When a document being edited is overwritten externally, keep the existing conflict handling: don't overwrite the on-disk content, prompt to reload or save as a copy.

## Migration (FORMAT 3 → 4)

1. Acquire the write lock, fully back up `index.json` and `docs/` (e.g. to `.quiet/before-folders-4/`).
2. Build the directory tree from the existing `folder` / `parent` fields, move `docs/<id>.*` to `<folder path>/<title>.<extension>`, escaping names per the rules below.
3. Rewrite all `doc-id` links as relative paths.
4. Generate a new `index.json`, and write `format.json` as 4.
5. If any step fails, roll back to the backup — never leave a half-migrated library.

Old versions will refuse to open FORMAT 4 (existing version check), so there's no risk of misreading. Migration needs round-trip tests like `library_round_trips`: before and after migration, every document's content, folder membership, and link targets must match.

## Open questions

1. **Title → filename.** How to escape `/ \ : * ? " < > |`, leading `.`, Windows reserved names (`CON`, etc.), and overly long titles; adding a suffix like ` 2` for name collisions within the same directory; whether renaming the title immediately renames the file (leaning toward: yes, immediately, while also rewriting referencing links). Markdown documents currently store the title in `Meta.name`; after migration the title is the filename.
2. **Where images go.** A single shared `assets/` for the whole library, or a per-document hidden `<title>.assets/` like local files use. The latter makes copying a single document easier, but renaming the title requires renaming the directory along with it.
3. **Where the recycle bin goes.** `.quiet/trash/`, or hand it off to the system trash. The former allows in-app restore; the latter is more familiar to users.
4. **Sync conflict copies.** iCloud, Dropbox, and similar services generate files like `Weekly Report (conflicted copy).md`; Git merges may leave conflict markers. How to detect and surface these during scanning.
5. **Local files (`Meta.path`).** `.md` files outside the library are still opened the existing way; #112's "open local folder" could reuse the same scanning and watching, treating an external directory as a second library root.
6. **Performance.** The cost of recursive scanning and watching for large libraries (thousands of documents); the existing `list()` cache that skips unchanged files by modification time and size should be kept.

## Additional requirements (2026-11-12)

- **What you see in Finder is the sidebar.** The directory pointed to by Settings › Library, once opened, shows exactly the same folders and title-named files as the sidebar, and the user can open notes directly; metadata like pinning and ordering lives only in `.quiet/`, with a separate entry point in settings to view it. The `preferences.json`, `shortcuts.json`, `themes/`, and `format.json` currently at the root all move into `.quiet/`.
- **Notes are self-contained.** A note's title (filename), body, and what it references (relative paths) can all be determined just by looking at that file; after deleting `.quiet/`, the whole library remains fully usable — only pinning, starring, and manual ordering are lost.
- **Sync-friendly.** On first launch the library can be placed in iCloud Drive, so:
  - State belonging only to this device (`opened` last-opened time, window size) doesn't go into the library — it's stored in the local app data directory, keyed by library path;
  - `.quiet/index.json` merges entry-by-entry by relative path, so two devices editing different documents don't conflict;
  - Scanning recognizes iCloud placeholder files (`.<name>.icloud`), triggering a download when needed rather than treating them as corrupted.

## Recommendations for open questions

1. **Filenames:** replace illegal characters with full-width equivalents or `-`, strip leading `.`, append `_` to Windows reserved names, truncate over 120 bytes; append ` 2`, ` 3` for collisions; rename the file immediately when the title changes and rewrite referencing links (coalesced with the existing 500ms save-debounce during editing, to avoid renaming too frequently).
2. **Images:** a hidden `.assets/` per folder. Copying a single document or an entire folder takes the images along; renaming the title doesn't require touching the image directory; and Finder isn't cluttered with extra directories.
3. **Recycle bin:** `.quiet/trash/`, keeping in-app restore and the existing recycle-bin UI; actual deletion only happens when the recycle bin is emptied.

## Phases

1. Library format and migration: new layout, scanning, path-based index, migration and rollback, round-trip tests.
2. Map the sidebar and move/rename/delete operations onto real files; convert links to relative paths with automatic rewriting.
3. Watch for external changes; surface conflict copies and broken links.
4. "Open local folder" (#112), move-into-library on import (#125).

## Later: the app's own iCloud container (not doing this now)

Don't mention iCloud on first launch. Later, for "guaranteed sync," go through iCloud Documents: an official build signed with iCloud entitlements gets `~/Library/Mobile Documents/iCloud~<container>/Documents`, and the library lives directly inside it, with files remaining plain `.md` / `.xml`. Don't use CloudKit — it stores data as cloud records, which violates "disk is the single source of truth."

Room is being left now to guarantee no breaking changes are needed later:

- The library location is tracked as a single path (`library.json`'s `path`); a library inside the container is just a normal directory as far as old versions are concerned. If info like "this is an iCloud container" needs to be recorded later, it can go into a new `#[serde(default)]` field on `Location`, which old versions will ignore.
- Layout and format are independent of storage location; state that belongs only to the local device (`opened`, window size) already doesn't go into the library.
- Placeholder files (`.<name>.icloud`) are recognized and trigger downloads during scanning, which also applies inside the container.

Still to add at that point: signing with container entitlements and a provisioning profile; checking `ubiquityIdentityToken` at launch and prompting if not signed in or if this app's iCloud is turned off; reading/writing through `NSFileCoordinator`; discovering remote changes with `NSMetadataQuery`; handling conflicts with `NSFileVersion` (done together with the phase-3 conflict-copy prompts). Self-built builds, Windows, and Linux don't have this entitlement and continue to use plain folders.
