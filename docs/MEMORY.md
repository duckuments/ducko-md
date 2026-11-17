# Memory experiment log

Origin: [#173](https://github.com/s1dashu/quiet-md/issues/173). On Windows, startup uses about 33MB, climbing past 400MB after viewing a few examples; another report said "it happens just from opening the home page."

## Conclusion

| Scenario (macOS footprint, MB) | M5 Pro before | M5 Pro after | M1 Pro before | M1 Pro after |
|---|---:|---:|---:|---:|
| Photo library startup | 230 | 184 | 156 | 110 |
| Photo library home page (8 cards, 4032×3024 photos) | **1160** | **207** | **1082** | **130** |
| Open a photo document | 873 | 222 | 797 | 147 |
| Back to a plain-text document | 825 | 222 | 749 | 147 |
| After 35 seconds | 377 | 202 | 208 | 127 |
| Guide library startup | 304 | 221 | 214 | 128 |
| Guide library home page | 324 | 228 | 239 | 138 |
| After viewing all guides | 328 | 255 | 236 | 179 |
| Guide library after 35 seconds | 298 | 233 | 222 | 157 |
| After editing/scrolling a million-character document (M5) | 346 | 281 | — | — |

For a production build (without devtools), stopped on the document page after launch, M5 Pro: photo library 106 → 76, guide library 165 → 109.

Main cause: document images and home-page card thumbnails were always decoded at full original resolution. A single 4032×3024 photo is 48.8MB of BGRA in memory, plus another copy in a GPU texture, totaling about 98MB; on a home-page card it's only drawn at about 190pt wide. The previous `DocImages` only released off-screen images after 30 seconds — on-screen images had no size cap.

Performance: frame times for typing, backspace, and scrolling on the same million-character document were within noise both before and after the change (see experiment 4).

Windows (Task Manager's "Memory" column, i.e. private working set; same photo library, driven by a key-press script, two runs with consistent results, second run reported):

| Scenario (MB) | Private working set before | after | Commit size before | after | GPU dedicated before | after |
|---|---:|---:|---:|---:|---:|---:|
| 6 seconds after launch (home cards decoding) | 497 | 35 | 1007 | 172 | 138 | 138 |
| Home page (8 photo cards) | **406** | **34** | 914 | 172 | **516** | **146** |
| Open a photo document | 129 | 46 | 352 | 195 | 233 | 158 |
| Back to a plain-text document | 82 | 34 | 305 | 183 | 233 | 158 |
| After 35 seconds | 35 | 33 | 163 | 160 | 138 | 138 |

The "before" numbers line up exactly with the issue's description of "over 400MB just from opening the home page." After the fix, the home page matches the startup numbers.

## Measurement method

- Script: [mem_probe.py](../tools/perf/mem_probe.py) replays a harness script, taking a process measurement (macOS) with `footprint` at every `shot`. A temporary library is used exclusively.
- Scenarios:
  - `photos`: [gen_photo_library.py](../tools/perf/gen_photo_library.py) generates 8 documents each containing one 4032×3024 photo (phone-camera size), plus one plain-text document named Notes. Script [photos.txt](../tools/perf/mem/photos.txt): launch → home page (8 photo cards) → open Photo 1 → open Notes → wait 35 seconds (cache expiry).
  - `guides`: the built-in guides (`shared-mock/guides/build.py`). Script [guides.txt](../tools/perf/mem/guides.txt): launch → home page → open the welcome page and the 5 pages under "Writing and Formatting" in sequence → open one plain-text guide → wait 35 seconds.
- Columns (MB):
  - `total`: overall footprint.
  - `pixels`: MALLOC_LARGE plus untagged VM, i.e. decoded images and other large buffers. Before the fix, this also included blocks already freed but not yet returned to the system by malloc; after the fix, large blocks are mmap'd directly, so they show up under untagged instead.
  - `small`: MALLOC_SMALL + TINY.
  - `gpu`: IOAccelerator + owned unmapped (graphics), i.e. textures and buffers.
  - `surface`: the window's IOSurface (swap chain).
  - `cg`: CG image.
- Machines:
  - M5 Pro: 48GB, macOS 26.5.
  - M1 Pro: 16GB, macOS 27.2 (the category name changed to "Malloc Large"-style wording; the script accounts for this).
  - Both are a 1280×800 pt @2x window, devtools build (off-screen rendering). Repeated runs of the same binary vary by within ±2MB on each metric; `surface` jumps between 32/48 depending on whether the swap chain has 2 or 3 images. devtools' off-screen rendering itself uses about 70MB more GPU on the M5, so startup numbers under devtools are higher than in a production build.

```sh
cargo build --release --features devtools
python3 tools/perf/gen_photo_library.py /tmp/q-photos
python3 tools/perf/mem_probe.py tools/perf/mem/photos.txt /tmp/q-photos /tmp/q-run/photos
```

## Experiments

All numbers are M5 Pro, devtools build, `total` MB. Each column accumulates on top of the previous one.

| Scenario | Baseline | 1a | 1b | 2 | 3 | 4 | 5 |
|---|---:|---:|---:|---:|---:|---:|---:|
| photos launch | 212 | 214 | 229 | 229 | 199 | 184 | 184 |
| photos home page | 1157 | 309 | 262 | 257 | 226 | 211 | 208 |
| photos photo document | 871 | 352 | 331 | 287 | 255 | 225 | 222 |
| photos after 35 seconds | 376 | 261 | 250 | 256 | 219 | 205 | 202 |
| guides home page | 335 | 288 | 276 | 276 | — | 228 | 228 |
| guides after viewing guides | 340 | 333 | 334 | 330 | — | 257 | 257 |

1a decodes at display size; 1b adds JPEG IDCT scaling; 2 drops the in-memory pixels after uploading; 3 is the app icon; 4 mmaps large blocks directly; 5 shrinks thumbnails by another step. (This column also includes the IDCT "within 15%" fix — the photo document's 331→287 mostly comes from that fix.)

### 1. Decode at display size (adopted)

`editor::image::picture(path, long_edge)` replaces `img(path).doc_picture()`:

- The cache key includes the target pixel size: long edge × window scale, rounded up to 128px below 1024 and 512px above, so dragging to resize an image doesn't re-decode every frame.
- After background decoding, `thumbnail_exact` (area averaging) shrinks it, and `RenderImage::with_scale_factor` keeps the natural size seen by layout unchanged.
- Target sizes in each place:
  - Body images: the display box;
  - Cover: the cover box;
  - Home-page cards: the drawn size on the card;
  - File preview: the preview box;
  - Lightbox: the current zoom level, which equals the original image at full zoom.
- While a new size is decoding, another already-available size of the same image is substituted in, avoiding a flash of white. When a different size of the same image already exists on screen, the old size is released at the next sweep.
- Animated GIFs still go through GPUI's loader, keeping all frames and original size.

Details:

- The first measurement of 1a still showed 611MB on the home page. Reason: when a thumbnail has no explicit width, `thumb_blocks` fills in the original image width (4032), which needed to first be clamped to the body column width of 690.
- Even after 1a, `pixels` was still about 70MB higher: two already-freed full-size RGB decode buffers (35MB each) were sitting in malloc's large cache, which `malloc_zone_pressure_relief` wasn't reclaiming at the time.
- 1b uses `jpeg-decoder`'s IDCT scaling, decoding directly at 1/2, 1/4, or 1/8 output — the full-size buffer never appears, and decoding photos is much faster too. IDCT can only scale by exact halves, so a tolerance of up to 15% below the target is allowed: a 2048 target uses 2016 instead of falling back to full size. Initially without this 15% allowance, a 2048 target fell back to full size, which was found using MallocStackLogging to trace a 36MB block back to `jpeg_decoder::compute_image`. PNG, GIF, CMYK, and 16-bit JPEG still go through `image`. New dependency: `jpeg-decoder 0.3` (MIT/Apache-2.0, rayon disabled, no transitive dependencies).
- Image quality:
  - Home-page card screenshots containing text actually look clearer than before. Previously, the GPU was doing bilinear downscaling on a texture 4.5x too large, causing aliasing.
  - Guide screenshots in the body (1600px, displayed at about 1380px) are nearly pixel-identical to before, with an average difference of 0.02/255.

### 2. Drop in-memory pixels after uploading to GPU (adopted)

GPUI only reads pixels the first time an image is placed into the atlas; every frame after that only uses the size. Vendor patch (PATCHES.md #24):

- Adds `hollow()` to `RenderImage`: same ImageId, same size, but no pixels.
- Once every window has drawn this image, the cache swaps it for the hollow version at the next sweep.
- If some window's atlas doesn't have it (a new window drawing it for the first time, or the atlas being rebuilt after a Windows device loss), `paint_image` skips it and records it in `Window::take_missing_images`, and the cache re-decodes it on the next frame.
- Animated images, and images shown in windows that don't report frames, aren't made hollow.

Confirmed with temporary logging that hollowing really happens (the 8 home-page thumbnails and the 2016×1512 photo document image were all swapped to hollow). On macOS, testing this item alone left footprint unchanged: the freed 12MB stayed in malloc's large cache (vmmap showed `MALLOC_LARGE (empty)`). Experiment 4 resolved this. Windows' heap returns blocks over 1MB directly to the system, and Task Manager's "Memory" column measures exactly that CPU-side private memory, so this item mainly matters for Windows.

### 3. App icon (adopted)

A 32MB block of "CG image" exists right from launch. MallocStackLogging showed it came from `set_app_icon` → `-[NSApplication setApplicationIconImage:]`: a 1024px PNG was treated as 1024pt, and AppKit drew it as a 2048² bitmap (32MB) on a Retina screen and kept it resident. Setting the NSImage's size to 256pt (the Dock's maximum zoom size) brought it down to just 2MB.

### 4. mmap blocks of 1MiB or larger directly (adopted, macOS / Linux)

[big_alloc.rs](../src/big_alloc.rs) acts as the global allocator: blocks of 1MiB or larger with alignment no more than 4096 use mmap/munmap, everything else goes to the system allocator. On free, pages are returned to the system immediately, matching the Windows heap's behavior. Both macOS's malloc and glibc keep freed large blocks around for reuse, so footprint didn't drop after images, decode buffers, or large text blocks were done with. After adding this, `pixels` is essentially zero, and the original `release_freed` now only matters for small blocks.

Frame time, million-character document, `readme-performance.txt`, P50 / P95 ms:

| Build | Continuous typing | Continuous backspace | Continuous scrolling |
|---|---|---|---|
| Baseline | 5.14 / 6.22 | 5.10 / 5.71 | 3.69 / 7.92 |
| 1–3 | 5.36 / 6.18 | 5.38 / 5.88 | 3.23 / 6.47 |
| 1–4 | 5.30 / 6.04 | 5.26 / 5.88 | 3.22 / 6.49 |

Other compile jobs were running on the machine during testing; the differences between the three groups are within noise.

### 5. Shrink thumbnails one step further (adopted)

Home-page cards decode at 0.75x the drawn size (`home.rs`'s `THUMB`). Image quality is slightly softer, but indistinguishable on a card. Since experiment 2 already dropped the in-memory copy, this item only saves about 3MB (GPU texture) on macOS.

### 6. Decode PNG while downscaling (adopted)

PNG previously went through `image`, decoding at full size, then `thumbnail_exact`: a 3000×1600 image started as an 8MB file and 14MB of RGB, and decoding two in parallel (`DECODERS`) added about 50MB momentarily. Now `decode_png` uses the `png` crate to read the file row by row, doing box averaging down to the target size (alpha-weighted when alpha is present), keeping only one row of the original and one row of accumulated values — the file is no longer read entirely into memory either. Interlaced PNGs, and ones that don't need shrinking, still go through `image`. New direct dependency: `png 0.18` (already a dependency of `image` at this version).

- The number of concurrent decode lanes changed from a fixed 2 to half the core count (2–4): each lane only holds one row of the original plus the shrunk result, so memory no longer grows noticeably with lane count. Pending decode requests are taken in the order submitted (i.e. draw order, top to bottom); when a request's turn comes up, if its window hasn't drawn it in the most recent frame (scrolled past), it's skipped and its slot dropped, re-queued the next time it's drawn.
- Time for all cards to finish (from opening a folder page or scrolling to the last card, to the last decode completing, logged timestamps): initial open 182–191ms → 180–192ms; two large scrolls 160–170 / 121–149ms → 149–168 / 134–145ms (15-core machine, 4 lanes after the change).
- Decoding itself: a synthetic library of 120 PNGs (1.2–6.6MP, 990MB total), decoded on two threads at 512px, peak RSS 68MB → 16MB, time 1.40s → 1.50s.
- In the app (harness opens a folder page with 40 cards and scrolls to the bottom, sampling `footprint` every 0.25s, `FDL_NOPARK`): peak 291/331MB → 289–300MB, steady state 216–225MB → 216–233MB. The pre-change high points were sporadic single samples (the instant of decoding); they're gone after the change.
- The remaining peak is unrelated to decoding: shrinking the same library's images to 1/20 size still showed a peak of 220–260MB and a steady state of 141–152MB. The amount above the steady state is a roughly 80MB block of "Owned physical footprint (unmapped) (graphics)" that appears during drawing activity and becomes reclaimable after one or two seconds of being idle; path textures aren't it (MSAA textures are confirmed Memoryless, 0 bytes, created once for the whole run) — the source hasn't been identified yet. The ~70MB by which the large photo library's steady state exceeds the small one's is the atlas texture for 120 thumbnails (IOAccelerator 31MB → 87–95MB), retained for 30 seconds after leaving the screen.

### Not adopted

- **Reducing the swap chain from 3 images to 2**: saves 16MB at 2560×1600. But at 120Hz, the CPU might wait on a drawable while encoding the next frame, possibly causing dropped frames; the harness's frame timing measures off-screen rendering and can't detect this, so it needs verification on a real screen before deciding.
- **Small-block malloc**: about 40MB at guide-library startup. Summed by call stack, live malloc totals 82MB, mostly one-time structures from startup (windows, fonts, menus) plus GPUI's per-frame text layout cache — no single app-layer owner stands out. The 40MB of "keyboard layout" seen in malloc_history turned out to be an mmap of an ICU data file (clean pages, not counted in footprint).

## Windows measurements

- Machine: Win11 build 26200, i5-13600KF, RTX 4070 Ti, 32GB, 1920×1080.
- Build: `cargo build --release --locked --features devtools`; "before" is main prior to the merge, "after" is this branch.
- Method: launch the app in a desktop session via a scheduled task (a window launched from an SSH session doesn't display), `FDL_SCRIPT` using only key presses: wait 7 seconds → `ctrl-alt-home` home page → `ctrl-j` jump to Photo 1 → `ctrl-j` jump to Notes → wait 40 seconds. Sampled at seconds 6, 13, 21, 28, 61:
  - Private working set: performance counter `Process\Working Set - Private`, i.e. Task Manager's "Memory" column;
  - Commit size: `PrivateMemorySize64`;
  - GPU: Dedicated / Shared Usage under "GPU Process Memory".
- A new library shows the home page immediately on first open, so the 6-second mark is already on the home page. Screenshots confirmed that the post-change card thumbnails and body photos both display normally.

Predictions matched measurements: experiments 1 and 2 are equally effective on Windows; experiments 3 and 4 only concern macOS / Linux.

## Device loss

A genuine device loss (`pnputil /restart-device` restarting the graphics driver) can't be tested at our layer on Windows: both the before and after builds crash on the first device loss, with `render_target_view` ending up null after a failed swap-chain resize in `directx_renderer.rs`, then `unwrap`ped. This is a pre-existing GPUI issue unrelated to this change, to be handled separately.

Our layer only cares about whether hollow images can recover after the atlas gets cleared. The devtools command `forget-uploads` removes every hollow image in the cache from the window's atlas and draws one frame, simulating the effect of an atlas rebuild:

- First version: on the home page, 2.5 seconds after clearing, the 8 cards were still blank. The missing state is recorded during frame N's draw, and only re-decoded on frame N+1; the home page has no blinking cursor or anything else requesting a new frame, so it had to wait for the user to move the mouse.
- Fix: `paint_image` requests the next frame as soon as it records a missing image. About 1 second after clearing, both the 8 cards and the body photo had recovered, on both macOS and Windows.
