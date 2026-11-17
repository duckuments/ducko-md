# Storage format

## Library directory (FORMAT 4)

Default location: `~/Documents/ducko` (a standalone folder under "Documents" that the user can see and back up directly). On first launch this can be changed to somewhere else; the chosen location is recorded in the local app data directory's `ducko/library.json` (`store::Location`, with just a single `path` field; any field added later must be `#[serde(default)]`). The environment variable `DUCKO_HOME` can temporarily point elsewhere, and tests always use it.

Disk is the sidebar: folders are real directories, and each document is one file named after its title (escaping rules in `layout::file_name_for`, with ` 2` appended for name collisions). See `docs/design/real-folders.md` for the design.

```
ducko/
├── Work/                folder
│   ├── Weekly Report.xml    DocxXML document
│   └── Proposal.md          Markdown document
├── Quick Note.xml
├── .assets/             pasted or inserted images, attachments; .assets/thumbs holds file preview thumbnails
└── .ducko/              the app's own data
    ├── format.json      {"format": 4}: directory layout version (store::FORMAT)
    ├── index.json       relative path → extra info (see below)
    ├── index.json.bak   previous version of index.json
    ├── settings.json    window-level UI settings (sidebar width, etc.)
    ├── preferences.json, shortcuts.json, emoji.json, themes/
    ├── .lock            write lock (flock): held briefly exclusive while rewriting the index, not a single-instance marker
    ├── trash/           recycle bin: index.json (id → Trashed) and the deleted files
    ├── history/<id>/    body version snapshots per document (see below)
    ├── history-trash/<id>/  version snapshots that went to trash along with the document
    └── before-folders-4/  backup of the old layout before migration, cleaned up after 30 days
```

Scanning recursively walks the library, recognizing `.md` and `.xml`, and skipping directories and files starting with `.` (`.ducko`, `.assets`, `.git`, `.obsidian`, etc.). Files placed by hand in Finder also show up. Window size belongs only to this computer, stored in the local app data directory, not in the library.

### Links and images

- Document links are written as paths relative to the current file: DocxXML uses `<cite type="doc" href="../Work/Weekly Report.xml">`, Markdown uses standard links. On read they're resolved to a runtime id, and converted back to a path on write, so links still work after the whole folder is copied elsewhere. `doc-id` is still recognized when reading old files.
- The `path` for images and attachments is relative to the library root (`.assets/…`).

### Index (.ducko/index.json)

Keys are relative paths, with folders ending in `/` (`"Work/"`). The value is `Attached`:

| Field | Meaning | When omitted |
|---|---|---|
| `id` | Runtime id, used by the editor, windows, undo; not the basis for links | Required |
| `created` / `modified` | Unix seconds | Required |
| `pinned` | Pinned | false |
| `starred` | Time starred, 0 means not starred | 0 |
| `order` | Sibling order within a folder | 0, not manually ordered |
| `root_order` | Top-level sibling order | not manually ordered |
| `width` | Page width: `standard` (820) / `wide` (1020) / `full` | standard |
| `catalogue_closed` | Table of contents collapsed | false |
| `reading` | Reading mode | false |
| `info` | Document info row: `{hidden, created, modified}` | only modified time shown |
| `refs_hidden` | Backlink list hidden | false |

Folder membership, name, and whether something is Markdown are all determined by the path, and aren't stored in the index. Deleting `.ducko/index.json` doesn't lose content, structure, or links — only extras like pinning, starring, and manual ordering. New fields always carry `#[serde(default)]`, and aren't written out when they hold the default value.

### Migrating from the old layout (FORMAT 3 → 4)

The old layout was flat `docs/<id>.xml`, with folders being just records in `index.json`. When `Store::open` encounters an old library:

- **Silent migration**: a local library migrates directly, and the changelog card afterward notes how many documents were migrated.
- **Consent first**: if the library is in a sync drive (iCloud, Dropbox, etc.), or shows signs of having been used on another computer (local file paths that don't exist, sync conflict copies), or the user previously restored the old layout, `Store::migration_state` returns `NeedsConsent` and the UI shows a prompt; choosing "later" asks again after a week.
- Before migrating, the entire old layout is moved into `.ducko/before-folders-4/`; if any step fails, it rolls back, leaving no half-migrated library. `Store::restore_backup` can revert to the old layout.
- The earlier FORMAT 2 converted parent-document structure into same-named folders, keeping `index.before-folders.json` before migrating.

Manual ordering in the sidebar and home page share `Meta::place()`: unpositioned entries sort by modification time descending first, positioned entries sort by their order number, with id as the final tiebreaker.

Old versions refuse to open FORMAT 4, so there's no risk of misreading it.

### Reliability guarantees

- **Atomic writes.** All files are first written to `<name>.tmp`, then `rename`d over the original. On a crash, disk holds either the old content or the new content, never a mix.
- **Recovery from index corruption.** Before every save of `index.json`, the old file is first copied to `.bak`. If parsing fails on read:
  - The corrupt file is renamed to `<name>.corrupt-<seconds>` and kept, so it's never overwritten;
  - `.bak` is used instead, with a one-time prompt to the user;
  - If no `.bak` is available, an empty index is used. Documents and folders are still on disk — only pinning, starring, and ordering info is lost.
- **Version marker.** When opening a library, if the version in `format.json` is newer than this build, it refuses to open and prompts an upgrade; the version is rechecked on every write after acquiring the lock. Any directory layout change that would cause old versions to misread it must bump `FORMAT`.
- **Hand-placed files show up too.** `.xml` (e.g. DocxXML exported from Lark) and `.md` files dropped into the library via Finder appear in the list.
- **Save failures are never silent.**

## DocxXML

DocxXML is Lark/Feishu's document XML format. `crates/core/tests/fixtures/all-blocks.xml` contains the canonical form for every block type; reading it in and writing it back out must be byte-for-byte identical (test `fixture_round_trips`).

### Write rules

- Each top-level block occupies one line. Children are on new lines, indented two spaces per level.
- **Blocks inside table cells are written on a single line, unindented.** Newlines within content (e.g. lines in a code block) are written as `&#10;`, to avoid confusion with layout newlines.
- An empty list item with child paragraphs is written as `<li><p></p>…`. Without this empty `<p>`, the child paragraphs would get merged into the list item on read.
- Folding is written as `folded="true"`. Icon and cover are written on `<title icon=… cover=… cover-x=… cover-y=…>`.

### Read rules

- Only **whitespace produced by source formatting** is discarded: a newline plus indentation, and the spaces/tabs before a nested block. All other whitespace is content — e.g. `<p> </p>` keeps that space.
- Unrecognized elements are preserved as-is and output unchanged: at the block level this is `BlockKind::Raw`.
- Inline objects (formulas, files, buttons, date reminders, @document, and unrecognized inline elements) each occupy one U+FFFC in the text, with an `Inline { kind, xml }` stored in the style:
  - `xml` is the original text as read, output unchanged, so unrecognized attributes are never lost;
  - `kind` is the type parsed on read; the view and export only look at this.
  - When the editor creates or modifies an object, it goes through `docxml::to_inline` to build canonical XML and read it back once, so the two always stay in sync, and saving and reading back never changes anything.

### Local extensions

Syntax not in DocxXML that this app adds on its own. Other tools don't recognize this syntax, and per the rules above will just treat it as unknown elements or attributes:

- `<countdown duration start-time notify color timing/>`: countdown. The cloud version's countdown is a widget, not represented in DocxXML.
- `<time … is-whole-day="true">`: all-day reminder. The model has `isWholeDay`, but there's no such attribute in the XML. A reminder without this attribute whose time is local 23:59 is also treated as all-day.
- Local files for images and attachments are written as `path="…"` (the cloud version uses `token` / `href`). `<figure><source path=… name=…/></figure>` represents a video or file.

### Known limitations

- **Trailing whitespace inside a cell.** If a block inside a table cell has nested children, trailing spaces or tabs at the end of its text are lost on read-back. Reason: old versions wrote cells with indentation, and reading must stay compatible with that, so whitespace before a nested block must be discarded. The fuzzer's `round_trip` applies the same preprocessing (`trim_before_nested`) to account for this.
- Blocks that can't have children (code blocks, dividers, images, etc.) will have any in-memory children discarded on write if they somehow have any. The editor guarantees this structure never arises (see the invariants in ARCHITECTURE.md).

## Version history

Each file under `.ducko/history/<id>/` is the on-disk body content **before** a particular save, written as-is, not re-serialized. The filename is `<13-digit millisecond timestamp>-<8-digit uuid>.xml|md`: the fixed-length timestamp sorts directly by filename, and the uuid suffix prevents two machines from generating the same filename in the same millisecond. A named version's name is stored alongside it in `<snapshot filename>.name` (one line of plain text, up to 40 characters); an empty name just deletes this file — the directory itself remains the single source of truth.

- **When generated**: after `Store::save_now` successfully writes new content, it writes a snapshot using the old text already in memory; it's skipped if the old text equals the new text, or if less than 5 minutes have passed since the last one. "Save current version" skips the interval limit, but still won't record the same text twice.
- **Retention policy**: 20 per document by default, adjustable to 1–30 in Settings › Library; the whole `.ducko/history/` also has a 200 MiB cap, with the oldest deleted first when exceeded. Both are trimmed after each write.
- **Deletion and restore**: `delete` moves each file (including `.name`) from `.ducko/history/<id>/` to `.ducko/history-trash/<id>/`; `restore` moves them back; `purge` and the recycle bin's 30-day expiry cleanup are what actually delete them. During trimming and 200 MiB budget eviction, the name file is deleted along with its snapshot.
- **Named versions**: hovering over a version in the drawer and clicking "Name" writes it to the adjacent `.name` file; the top of the drawer can show "named only," to find milestones among dozens of automatic snapshots.
- **Restoring a version**: the current on-disk content is first force-saved as a snapshot, then the editor writes the historical version through the normal `save` path — so restoring itself leaves a way back. "Save as copy" creates a brand-new document with a new id, titled with that version's name or time (`Product Notes (draft)`), so that side-by-side it's clear which version is which.
- **Preview**: clicking a version switches the body area to a read-only snapshot of that content (the editor enters reading mode via `Editor::set_preview`), with a banner at the top indicating "which version is being viewed / show changes / restore this version / back to current." On returning, the full editor state from before entering preview (document, cursor, selection, undo stack) is restored, so switching back and forth never loses content. During preview, both `Store::flush` and `sync_document` are skipped entirely — the historical content is never written back.
- **Showing changes**: compares a version against its **immediately preceding** snapshot block by block (`changes::block_changes`, a block-level LCS). New blocks get a light green background, rewritten blocks get a light yellow background; container blocks (quote, callout, split) have their background drawn on their own child rows. Deleted blocks aren't present in this version — switch to the previous one to see them. Block ids are generated fresh at parse time, so the markup must use the same parse result as the rendering.
- **Per-row change amount**: each snapshot in the drawer is followed by `+N lines −M lines`, computed from a line-level LCS (`diff`) between adjacent versions; the oldest one has no predecessor to compare against and is labeled "earliest version."
- **Failure doesn't affect the body**: a failed snapshot write only logs a warning — saving the document still succeeds normally.
- **Compatibility**: old versions don't read `.ducko/history/`, and the extra directory doesn't cause them to misread the index or body content, so `store::FORMAT` doesn't need to bump. When an old version writes into a new library that has history, history simply stops updating — it isn't corrupted.
- **Correspondence**: snapshots are matched to documents by the runtime `id` in the index, so history survives renaming and moving folders; after deleting `.ducko/index.json`, ids are reassigned and old snapshots no longer correspond to any document.
- **Attachments**: `.assets/` currently has no cleanup mechanism, so images and attachments referenced by historical content are always available. If GC is introduced in the future, it must also scan all historical snapshots, trashed documents, and recovery copies.

## Save failures and recovery

Writes use a uniquely named temporary file in the same directory, synced before being renamed into place; on Unix, the parent directory is synced too. If acquiring the library's write lock fails, modification is refused, though notes can still be listed read-only. If another instance's modification to the body is detected, disk content is never overwritten — it can be reloaded or saved as a copy.

If saving fails during system shutdown, the content is saved instead to the local app data directory at `ducko/recovery/<library-id>/`. The next time the same library is opened, a recovery copy is created, and the recovery file is removed once the write succeeds. This is the last-resort fallback for a failed shutdown, not a replacement for per-document version history (see above section).

When exporting local resources, canonicalized paths are checked, rejecting files outside the library and symlinks that escape it. The inline-image limit for HTML embedding is 20 MiB, and the per-resource limit for Markdown copy is 256 MiB; missing, rejected, or over-limit Markdown resources count as export warnings. Both inline and block attachments are exported as links with the file copied alongside.
