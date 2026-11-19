# Document themes (CSS)

Each ducko document theme is a CSS file. Choosing a theme means choosing that CSS; clicking "Edit CSS" in Settings › Appearance opens it in your editor, and once you save it, documents in every window are immediately repainted with the new style.

The Feishu (Lark) theme is the default on first launch; an existing theme choice is kept. The dropdown lists Feishu, Craft, Notion, Obsidian, and ducko, showing options only; selecting one updates the page preview below.

- The built-in themes (Feishu, Craft, Notion, Obsidian, ducko) live in `assets/themes/`, each one **with every property's current value written out in full**, so they can be used directly as a reference.
- When editing a built-in theme, the app first copies it in full to `themes/<id>.css` in the library; this copy then replaces the built-in version from then on (shown as "Modified" in the menu; "Restore default" deletes the copy).
- "Duplicate as new theme" generates `themes/<id>-copy.css`; renaming it makes it your own theme. You can also just drop someone else's `.css` file into the `themes/` folder, and the app will pick it up automatically.
- Invalid values, or unrecognized selectors or properties, don't break the theme: they're skipped and listed as "not applied" in Settings.

## File structure

```css
:root {
  --theme-name: "My Theme";          /* display name, required */
  --theme-name-en: "My theme";       /* name shown under an English UI */
  --theme-author: "Your name";
  --theme-description: "A short description";    /* kept as metadata, not shown in the menu */
  --theme-base: craft;               /* how blocks are drawn: craft or feishu */

  --accent: hsl(254, 80%, 68%);      /* custom variable, referenced below via var() */
  --color-red: #d44c47;              /* named color, see below */
}

.page { color: #37352f; font-size: 16px; line-height: 1.5; max-width: 708px; }
h1 { font-size: 1.875em; font-weight: 600; margin-top: 32px; }
blockquote { border-left: 3px solid var(--accent); }
```

- Supports `/* comments */`, `var(--x, fallback)`, and comma-separated multiple selectors; at-rules like `@media` are ignored in full.
- Lengths: `px` (= pt), `em` (relative to body font size; otherwise relative to the current element's font size), `rem` (16), unitless `line-height` (a multiplier).
- Colors: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, `hsl()`, `hsla()`, `transparent`, `white`, `black`; a color property set to `inherit` follows the text color.
- `font-family` takes the first installed font in the list; `system-ui`, `-apple-system`, `sans-serif` are the system font, `ui-monospace`, `monospace` are the system monospace font.

## Selectors and properties

A theme with `--theme-base: feishu` draws blocks the Feishu way, and only uses properties marked "Feishu"; other properties are listed as "not applied" if set.

| Selector | Properties | Description | Feishu |
|---|---|---|---|
| `.page` (`body`) | `background-color` `color` `font-family` `font-size` `line-height` `caret-color` | Page and body text | ✓ |
| `.page` | `max-width` | Width of the body column at "Standard" page width; "Wide" and "Full" are unaffected | ✓ |
| `::selection` | `background-color` | Background of selected text | ✓ |
| `p` | `margin-bottom` | Spacing between blocks | ✓ |
| `.title` | `font-family` `font-size` `line-height` `font-weight` `color` | Document title | ✓ |
| `h1`–`h9` | `font-family` `font-size` `line-height` `font-weight` `color` `margin-top` `margin-bottom` | Headings at each level | font size, line height, weight, spacing above/below |
| `strong` (`b`) | `font-weight` `color` | Bold | |
| `a` | `color` `text-decoration` (`underline` / `none`) `text-decoration-color` | Links; with `underline`, the underline is always drawn | `color` only |
| `a:hover` | `color` | Hovered link | ✓ |
| `code` | `color` `background-color` `border-color` `border-radius` `padding-left` | Inline code | |
| `pre` | `font-family` `font-size` `line-height` `color` `background-color` `border-color` `border-radius` | Code block | color and font |
| `.line-numbers` | `display` (`block` / `none`) `color` `background-color` | Code block line numbers | color |
| `.token.comment` `.token.keyword` `.token.tag` `.token.string` `.token.number` `.token.boolean` `.token.class-name` `.token.attr-name` `.token.function` | `color` | Code highlighting (follows Prism's class names) | ✓ |
| `blockquote` | `border-left` (or `border-left-width` / `-color`) `padding-left` `color` `font-style` | Quote: bar, indent, text | bar color only |
| `ul` | `padding-left` | Child indent | |
| `ul li::marker` / `ol li::marker` / `li::marker` | `color` | Bullet / number color | |
| `.task` (`input[type=checkbox]`) | `width` (= height) `border-radius` `border-color` | Checkbox | `border-color` only |
| `.task:checked` | `background-color` `color` | Background when checked, checkmark color | |
| `.task-done` | `color` `text-decoration` (`line-through` / `none`) | Text of a completed task | `color` only |
| `hr` | `border-top` (or `border-top-style` `-width` `-color`) | Divider: `dotted` / `solid` / `dashed` | color only |
| `table` (`td`) | `font-size` `line-height` `border-color` `border-radius` | Table text and lines | `border-color` only |
| `th` | `background-color` | Header background | ✓ |
| `.callout` | `border-radius` `border-width` `padding` (top / left) | Callout | |

### Named colors in `:root`

Text color, background color, and callout color in a document are stored by name (red, orange, yellow…); the theme decides what color they actually display as:

| Variable | Use |
|---|---|
| `--color-gray` … `--color-purple` | Text color (gray red orange yellow green blue purple) |
| `--highlight-light-red`, `--highlight-red` … | Text highlight (one shade lighter with `light-*`, one shade darker with no prefix; gray also has `medium-gray`) |
| `--background-light-red`, `--background-medium-red` … | Block background |
| `--callout-light-red`, `--callout-medium-red` … | Callout background |
| `--border-red` … | Callout border |

### Other details: `--token-*`

There are also some interface-level color details in the document (block hover, block selection, table toolbar, size-picker cells, etc.), overridable via `--token-<name>` in `:root`; see the "Other interface details" section at the end of a built-in theme file for the full list and current values.

## Scope

- A theme only changes **the document itself**: the sidebar, menus, settings, and the rest of the UI are unaffected by the theme.
- No dark mode yet: the page can be set to a dark color, but the surrounding chrome stays light.
- A block's overall structure (e.g. a code block's toolbar, a table's row/column controls) isn't determined by CSS.
