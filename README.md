<h1 align="center">ducko</h1>

<p align="center"><b>A fast, quiet, local notes app.</b><br>
Drawn natively with Rust + GPUI. Your notes stay on your computer.</p>

<p align="center">
  <img src="docs/images/home.png" width="100%" alt="The ducko home screen: a document tree on the left and notes in a grid on the right.">
</p>

## Why ducko

- **Fast.** The whole interface is drawn on the GPU by Rust and [GPUI](https://www.gpui.rs/). No WebView, no Electron.
- **Local-first.** No account, no cloud. Your library is an ordinary folder on your computer — each note is a file named by its title (`.xml`, or `.md` for Markdown). Manage it with Git, iCloud or any sync tool you like.
- **Rich text.** Notes are saved in the Feishu (Lark) DocxXML format, so copy-paste, import and export keep their structure.

## Features

- **Editing**: headings, lists, to-dos, quotes, code blocks with syntax highlighting, LaTeX math, tables (with merged cells), callouts, columns, images, attachments and covers. Markdown shortcuts (`#`, `-`, `[]`, `>`) and a `/` slash menu.
- **Version history**: each note keeps snapshots as it saves, with a one-click restore.
- **Organizing**: a document tree, pinning, favorites, full-text search and a trash.
- **Import and export**: import Markdown folders and Feishu (Lark) DocxXML; export Markdown and PDF, or print.
- **Themes**: four built-in styles (Craft, Feishu, Notion, Obsidian), each with a dark variant, or write your own in CSS — see [docs/THEMES.md](docs/THEMES.md).

The interface is available in English, Simplified Chinese, Japanese, Korean, Spanish, French and Arabic.

## Build

```sh
cargo run --release
```

Building, testing and code conventions are in [CONTRIBUTING.md](CONTRIBUTING.md).

## License

The code is licensed under [MIT](LICENSE). Third-party code, fonts and assets are listed in [THIRD_PARTY.md](THIRD_PARTY.md).

Not covered by the MIT license, and to be replaced in forks:

- The Nucleo icons in `assets/icons/` are under a commercial Nucleo license. Do not extract or redistribute them on their own.
- The name "ducko" and the app icon.
