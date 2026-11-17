# Architecture

Quiet.md is a single-process GPUI application (Rust, about 40k lines). A document is a tree of blocks in memory (`model.rs`), and a DocxXML file on disk (`docxml.rs`). The editor paints this tree as a virtualized list.

The repo is a Cargo workspace split into two crates:

- **`crates/core` (`fdl_core`, about 10k lines)**: the document engine. Includes the model, DocxXML, the editing kernel and undo, the on-disk library, Markdown / HTML import, and Markdown export. It doesn't depend on GPUI and can be compiled and tested on its own (`cargo test -p fdl_core`); the fuzzer also lives here.
- **The root crate (`quiet_md`)**: the GPUI application — windows, views, rendering, menus, and overlays. `main.rs` re-exports `fdl_core`'s modules at the crate root, and `editor/mod.rs` re-exports `editor::core` and `editor::table_ops`, so the app can still write `crate::model`, `super::core`.

Dependencies only flow from the app into core. Modules in core only reference other modules in core.

```
main.rs ─ window
  ├─ onboarding.rs  first-launch onboarding page; once a library is chosen, the same window switches to Workspace
  └─ app.rs  Workspace (the window's root view)
       ├─ store.rs        the library: real folders + .quiet/index.json (see FORMAT.md)           [core]
       ├─ sidebar / home / search / trash / tree / docmenu / docinfo / modes
       └─ editor::Editor (view.rs)   the currently open document
            └─ editor::Core (core.rs) document + selection + undo stack                        [core]
```

## Layers

| Layer | File | Responsibility | Rule |
|---|---|---|---|
| Model | `model.rs` | `Document` / `Block` / `BlockKind` / `RichText` (text + style runs) / inline objects `Inline { kind, xml }` | Pure data. `BlockKind::nests()` decides which blocks can have children |
| Format | `docxml.rs` | DocxXML ⇄ `Document` | Unknown elements are preserved as-is as `Raw`; inline objects keep their original text, with the type parsed once on read; the output format is stable, see FORMAT.md |
| Editing kernel | `editor/core.rs`, `editor/table_ops.rs` | All operations that modify a document: typing, deletion, merging, indenting, conversion, dragging, tables, folding, inline objects, undo | Lives in `fdl_core`, doesn't touch GPUI, can be unit-tested and fuzzed directly |
| Editing view | `editor/view.rs` + the rest of `editor/*` | Keyboard/mouse events → Core calls; overlays, menus, and per-block interaction | Document changes only go through Core (see below) |
| Rendering | `editor/render.rs`, `editor/text.rs` | block → GPUI elements; `text.rs` is a self-drawn rich text element | Read-only on the document |
| App | `app.rs` and other top-level modules | Sidebar, home, search, trash, document tree, import/export, printing | Reads/writes disk through `Store` |

## Editing flow

The sequence for a single edit is fixed:

1. The view receives an event and calls `core.checkpoint(kind)`. This records an undo snapshot, increments `generation`, and sets `dirty`. Consecutive typing (same block, less than 700ms apart) is merged into one step.
2. Core's operation methods are called to modify `core.doc` / `core.sel`.
3. The editor is notified according to scroll intent:
   - Cursor-targeted operations like typing and navigation call `Editor::changed()`, which first calls `core.reveal_caret()` to expand any collapsed heading or list item hiding the selection, and requests a scroll to the cursor;
   - Operations with an independent target, like clicking a task checkbox or resizing a column, call `changed_preserving_viewport()`, which preserves the cursor/selection and viewport. If folding moved the cursor into a heading, `changed()` is still used to show the migrated cursor;
   - Both paths fire `EditorEvent::Changed` when there's a change, which also refreshes search results, the slash menu, and the @ panel.
4. On receiving `Changed`, `Workspace` calls `schedule_save`, which flushes to disk after 500ms via `flush()`. If the write fails, the change stays unsaved, the top bar shows an error, and switching documents, creating a new one, or quitting will prompt first. The save-failure dialog supports retrying, reloading disk content, or saving a recovery copy; reloading also resets the editor and the state of other windows on the same document. Quitting checks all windows, resolves a single conflict and rechecks; if conflicts remain, it won't quit. If saving still fails during system shutdown, the content is stashed in the local app data directory at `quiet-md/recovery/<library-id>/`, and restored as a new document (without overwriting the original on disk) the next time the same library is opened.

After a successful write, `Store::save_now` also stores a version snapshot of the old in-memory content under `history/<id>/`: throttled to 5 minutes, 1–30 per document, with a 200 MiB cap for the whole library. A snapshot write failure is only logged as a warning and doesn't affect saving the document. Restoring a version first forces a snapshot of the current content, then proceeds through the normal `save` path.

The version history UI follows the Google Docs / Lark pattern: a drawer on the right, the body switching in place to a read-only preview of the selected snapshot, with changes shown as a toggleable highlight layer. For this, `Workspace` has an extra `preview: Option<Preview>` field, which holds the full `Core` (document, cursor, selection, undo stack) from before entering preview, and restores it as a whole on return. During preview, `flush`, `schedule_save`, and `sync_document` all short-circuit: the editor holds historical content, and writing to disk or syncing to sibling windows would otherwise treat it as the current document.

Diffing happens at two levels: `changes.rs` does a block-level LCS to answer "which blocks are new in this version, which were rewritten," used to color the body; `diff.rs` does a line-level LCS, used only to compute the drawer's per-row `+N lines −M lines`. Rules are in FORMAT.md.

Core's invariants are checked by `crates/core/src/editor/fuzz_tests.rs` after every random edit step:

- The selection lands on a visible caret stop, with both ends in the same scope (same cell or same body);
- Block ids are unique, runs don't go out of bounds or overlap; inline-object runs contain only U+FFFC;
- Column ratios in a split sum to 1, merged table cells don't overlap;
- Only blocks where `nests()` is true, and containers (quote, callout, split, column), have children;
- Code blocks have no styling and no inline objects;
- Saving and reading back produces an identical result;
- Undoing all the way and redoing all the way returns to the same state.

### Exceptions for direct modification

Structural changes (adding/removing or moving blocks, merging/splitting text) all go through Core. A few places that **only modify a single block's attributes** still write directly via `core.block_mut(id)` from view modules, with `checkpoint` / `changed` wrapped around them the same way:

- Image size, cropping, caption (`image.rs`)
- Countdown settings (`countdown.rs`)
- Code block name (`caption.rs`)
- Icon, cover (`cover.rs`)
- Title renaming (`view.rs`)
- Several checkbox and fold toggles (`view.rs`, `render.rs`)

These don't change the tree structure, so they don't break the invariants above. Any new structural operation must go into Core, with a corresponding random operation added to the fuzzer's `step`.

## Undo

- Each step is a snapshot of the entire `Document` (`Snapshot`), stored in a `VecDeque`. The cap is 500 steps, and roughly 256MB; even for very long documents, at least 20 steps are always kept.
- Document icon and fold state are excluded from history: on undo/redo, `keep_outside_history` preserves their current values.
- `generation` is incremented on every change (including folding). Render cache, table of contents, and word count all use it to decide whether to recompute.

The snapshot approach is simple and reliable, at the cost of copying the whole document on every step for long documents. In measured history on a million-character document, the p50 for processing one input-sync frame is 5.06ms; this excludes on-screen presentation latency — see the README for the test build and methodology.

## Rendering and virtualization

- The page is a GPUI `List`. Items in order are: the header area (`Item::Header`), each visible top-level block, and `Item::Below` (backlink list).
- Children of blocks like text, headings, and lists each become their own row (`flattens`). Children of container blocks are drawn inside the container's own row.
- `sync_items` computes a key for each row, `hash_block`. It's compared against the previous frame's keys, and only `splice`s the range that changed outside the common prefix/suffix, so only changed rows get re-measured for height.
- `hash_block` only takes the length of text, not its content. Reason: the list re-measures visible rows every frame anyway, so a same-length edit (e.g. ⌘B on a whole paragraph) at worst makes an off-screen row's cached height briefly stale. Hashing the text content itself would add about 3ms per keystroke on a million-character document. The measurement is recorded in a comment in `render.rs`.
- The scrollbar uses `ListState::estimated_scroll_metrics`, added via a vendor GPUI patch: unmeasured rows are estimated using the average row height (see vendor/PATCHES.md).

## Memory and caching

- Document images use a shared cache, protecting images still displayed in any window. When the off-screen decode cache exceeds 64 MiB, the least-recently-used entries are evicted first; other off-screen entries are reclaimed after 30 seconds; checked every 2 seconds, which also cleans up the GPU atlas. At most two images decode concurrently; already-stale queued requests skip decoding. Original images aren't downsampled, and animated images keep their existing decode/playback path.
- Images with a specified width/height only start loading once they're actually visible and about to be painted, to avoid decoding body images on the home page early just because a card got clipped. The home page caches the most recently used 128 document previews.
- Full-text search's plain text is shared via `Arc<str>` across the parse cache, window list, and file history. After a successful save when switching documents, if no other window is using the old document, the old document's full XML conflict-detection baseline is released; it's rebuilt when reopened.
- Metal / DirectX path intermediate textures are created on demand and reclaimed on subsequent frames without a path. Apple GPU multisampling uses on-chip memory, keeping the antialiasing sample count unchanged.

## Module map

Document engine (`crates/core/src/`):

| Module | Contents |
|---|---|
| `model.rs` | Document model, including inline object types, `display_title` |
| `docxml.rs` | DocxXML parsing and serialization; `to_inline` constructs inline objects |
| `store.rs` | Library read/write, index recovery, trash, settings; `image_path` |
| `paste.rs` | HTML / Markdown → blocks (shared by paste and import) |
| `markdown.rs` | Markdown export |
| `lang.rs` | Code language table |
| `palette.rs` | Text color, background color, callout color palettes (RGBA, no GPUI dependency; `theme.rs` converts to GPUI colors) |
| `time.rs` | Date conversion, local timezone, current time |
| `editor/core.rs` | Editing kernel |
| `editor/table_ops.rs` | Table operations and cell selection (`CellSel`) |
| `editor/fuzz_tests.rs` | Random-edit fuzzer |
| `versions.rs` | Body version snapshots: directory listing as index, throttling, quota, trash integration |
| `diff.rs` | Line-level LCS diff between versions (no third-party dependency) |
| `changes.rs` | Block-level LCS between versions: which blocks are new or rewritten, used to color snapshots |

Application top level (`src/`):

| Module | Contents |
|---|---|
| `main.rs` | Entry point; `FDL_SCRIPT` script mode (`devtools` feature only); re-exports `fdl_core`'s modules |
| `guides.rs` | Seeds the built-in "Getting Started" guide when the library is empty (only once) |
| `app.rs` | `Workspace`: two-pane window, open / new / save / delete, save state, reminder scheduling |
| `html.rs` / `printing.rs` / `transfer.rs` | HTML / PDF export, printing, import/export UI |
| `clipboard.rs` | Reading HTML from the system clipboard, plus a simulated clipboard for script mode |
| `sidebar.rs` / `tree.rs` | Left sidebar and document tree (sub-documents, move to) |
| `home.rs` / `trash.rs` | Home page, trash |
| `search.rs` | Search popup, advanced search |
| `docmenu.rs` / `docinfo.rs` / `modes.rs` | "…" menu, document info, edit/read-mode toggle and shortcuts panel |
| `theme.rs` | Design tokens: `craft` color scale, semantic colors `Theme::*`, display colors for named colors, shadows `shadow_*`, type scale, note panel dimensions |
| `ui.rs` / `input.rs` | Shared widgets (menu shell, menu row, white pill button, tooltip, toast), single-line input field |
| `highlight.rs` | Code highlighting |
| `assets.rs` | Icons, fonts, and cover art library baked into the binary |
| `devtools.rs` | Script driver, only compiled in with `--features devtools` (see tools/harness/README.md) |
| `history.rs` | Version history drawer: list grouped by day, in-place read-only body preview, change display, naming versions, restore / save-as / delete |

Editor (`src/editor/`):

| Module | Contents |
|---|---|
| `view.rs` | The `Editor` entity: state, keyboard/mouse handling, `changed()` |
| `keymap.rs` / `menus.rs` | Keybindings, slash menu and conversion menu data |
| `render.rs` / `text.rs` | Page rendering, rich text elements |
| `overlays.rs` | Handles, slash / + menu, block menu, floating toolbar |
| `drag.rs` / `gap.rs` | Dragging blocks, clicking the gap between blocks |
| `table.rs` / `grid.rs` | Tables (UI part; operations in core's `table_ops.rs`), columns |
| `image.rs` / `file.rs` / `cover.rs` | Images, video or file, icon and cover |
| `math.rs` | Formulas (katex-rs generates HTML, then it's laid out manually) |
| `link.rs` / `links.rs` | Link panel and cards; @document and backlink list |
| `button.rs` / `countdown.rs` / `reminder.rs` / `subpages.rs` | Button, countdown, date reminder, sub-page list |
| `emoji.rs` / `emoji_picker.rs` | Emoji data and picker |
| `catalogue.rs` / `find.rs` / `reading.rs` / `scrollbar.rs` / `caption.rs` | Table of contents, find/replace, reading mode and info row, scrollbar, code block name and image caption |

## Interface language

The UI supports Simplified Chinese, English, Japanese, Korean, Spanish, French, and Arabic (`settings::Lang`), with the preference stored in `Preferences::language`; for old files without this field, `Preferences::english` decides it, and `english` is still written out so that old versions display other languages as English. Static copy uses the unified mapping of `settings::text` / `label`, paired copy uses `settings::tr`, and copy with parameters uses `localized_format!`. Menu tables keep stable original labels that are translated at display time; search matches both the English label and the original keywords. Input fields keep their original placeholder, translated at render time, and all windows refresh when the language changes. Arabic currently only has translated copy — the layout is still left-to-right, with no RTL mirroring.

The Chinese source text is the translation key: Japanese, Korean, Spanish, French, and Arabic each look up the Chinese text in `src/l10n/ja.rs`, `ko.rs`, `es.rs`, `fr.rs`, `ar.rs` respectively, and `tr`, `text` / `label`, and `localized_format!` all check these first; anything missing from a table falls back to English, so adding new Chinese copy doesn't require also writing these other languages. The translation template for `localized_format!` reads parameters back out of the formatted Chinese text and refills them (`l10n::refill`), so call sites don't need to change. Places that don't go through the macro and need to specify a language (the date-reminder tests) use `settings::localized`. `settings::english()` means "not Chinese."  `python3 tools/l10n/keys.py --missing ja` (or `ko`, `es`, `fr`, `ar`) lists keys still missing from that language's table; `l10n`'s tests check that every key in a table still exists in the source and that every template placeholder originates from the Chinese source text.

User titles, body text, attachment names, and saved button text are not translation keys; only UI defaults like an empty title vary with language. Core's storage diagnostics are translated at the app layer using known templates, preserving paths, document IDs, and the original system error text — the document engine itself doesn't depend on the interface language. The localization regression check covers UI translation keys, menu tables, date reminders, and the preservation of user content and diagnostic parameters.

## Folder and file-operation undo

A folder is a `Meta::folder` entry in the index; only a folder can be a parent. Core's `store/folders.rs` handles old-tree migration, hierarchy validation, and sibling ordering; the app's `folders.rs`, `sidebar_drag.rs`, and `home_folder.rs` handle the folder page, drag-and-drop, and animation.

Undo for create, copy, rename, move, delete, and restore is kept separate from document-body undo. The storage layer records actually-successful file operations within the write lock, to avoid folding in changes from other instances; after an async drag is committed to disk, the preview record is replaced with the actual record. Undo checks file state and hierarchy first; body content is read from the current disk state, not restored from an old content snapshot. When a management operation changes a title, the cursor bounds in the editor and in its history snapshots are corrected to match.
