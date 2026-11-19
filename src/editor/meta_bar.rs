//! The metadata bar under the title: `date` / `tags` / `status`, parsed
//! from a Markdown file's YAML front matter (`ducko_core::markdown`'s
//! `parse_front_meta` / `patch_front`). Markdown-only, like the front
//! matter itself — a DocxXML document has none.

use super::overlays::at;
use super::view::{Editor, EditorEvent, Menu};
use crate::input::TextFieldEvent;
use crate::model::FrontMeta;
use crate::editor_theme::{self as theme, Theme};
use crate::editor_ui::{MENU_INSET, MENU_PAD_Y, MENU_ROW_H, ROW_RADIUS, TipExt, icon, menu, menu_row};
use gpui::{prelude::FluentBuilder, *};
use std::hash::{Hash, Hasher};

const STATUS_PRESETS: [&str; 3] = ["active", "in-progress", "done"];
/// 22 tall pill chips (date / status / tags), 6 apart, under the title.
const CHIP_H: f32 = 22.;

fn hash_str(s: &str) -> u64 {
    let mut h = rustc_hash::FxHasher::default();
    s.hash(&mut h);
    h.finish()
}

/// A readable, deterministic chip colour for a tag or status name: one of
/// `palette::HUES`'s six colourful hues (not gray), picked from its text so
/// the same name always gets the same colour.
fn hue_of(s: &str) -> &'static str {
    let hues = &crate::palette::HUES[1..];
    hues[(hash_str(s) as usize) % hues.len()]
}

/// A saturated chip: the hue's own swatch behind white text (not the pale
/// `light-{hue}` tint used for text highlights elsewhere).
fn chip_colors(hue: &str) -> (Hsla, Hsla) {
    (theme::bg_color(hue).unwrap_or(Theme::fill_hover()), gpui::white())
}

/// `"2026-11-19"` → `(2026, 11, 19)`; permissive about leading zeros, strict
/// about the shape (anything else, e.g. a hand-written front-matter date
/// ducko doesn't recognise, just won't preselect a day).
fn parse_ymd(s: &str) -> Option<(i64, u32, u32)> {
    let mut it = s.trim().split('-');
    let y = it.next()?.parse().ok()?;
    let m = it.next()?.parse().ok()?;
    let d = it.next()?.parse().ok()?;
    it.next().is_none().then_some((y, m, d))
}

impl Editor {
    // ---- committing edits (file-level, not undone, like the doc icon)

    fn set_meta(&mut self, f: impl FnOnce(&mut FrontMeta), cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let mut meta = self.core.doc.meta.clone();
        f(&mut meta);
        if meta == self.core.doc.meta {
            return;
        }
        self.core.doc.meta = meta;
        self.core.generation += 1;
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    fn set_date(&mut self, y: i64, m: u32, d: u32, cx: &mut Context<Self>) {
        self.set_meta(|meta| meta.date = Some(format!("{y:04}-{m:02}-{d:02}")), cx);
    }

    fn commit_status(&mut self, status: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.set_meta(|m| m.status = status, cx);
        self.close_menu_focus(window, cx);
    }

    fn add_tag(&mut self, tag: &str, cx: &mut Context<Self>) {
        let tag = tag.trim().to_string();
        if tag.is_empty() {
            return;
        }
        self.set_meta(|m| if !m.tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) { m.tags.push(tag) }, cx);
    }

    fn remove_tag(&mut self, tag: &str, cx: &mut Context<Self>) {
        self.set_meta(|m| m.tags.retain(|t| t != tag), cx);
    }

    // ---- date popover: a calendar, no typing

    /// The note's date, or (for display only, nothing written) the day it
    /// was created — "the default date for new notes is the date of create".
    fn display_date(&self) -> (Option<(i64, u32, u32)>, (i64, u32, u32)) {
        let set = self.core.doc.meta.date.as_deref().and_then(parse_ymd);
        let (cy, cm, cd, ..) = crate::time::civil(self.created);
        (set, (cy, cm, cd))
    }

    pub(super) fn open_date_menu(&mut self, pos: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.menu, Some(Menu::Date { .. })) {
            return self.close_menu_focus(window, cx);
        }
        let (set, created) = self.display_date();
        let (y, m, _) = set.unwrap_or(created);
        self.menu = Some(Menu::Date { pos: self.rel(pos), view: (y, m) });
        window.focus(&self.focus);
        cx.notify();
    }

    pub(super) fn date_menu(&self, pos: Point<Pixels>, view: (i64, u32), _window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (vy, vm) = view;
        let (set, created) = self.display_date();
        let (ty, tm, td, ..) = crate::time::civil(crate::time::now());
        let first = crate::time::days_from_civil(vy, vm, 1);
        let start = first - i64::from(crate::time::weekday(vy, vm, 1));
        let nav = |key: &'static str, name: &'static str, delta: i32| {
            div()
                .id(key)
                .size(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(ROW_RADIUS))
                .cursor_pointer()
                .hover(|s| s.bg(Theme::fill_hover_strong()))
                .child(icon(name, 16., Theme::icon()))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |e, _, _, cx| {
                        let k = vy * 12 + i64::from(vm) - 1 + i64::from(delta);
                        e.menu = Some(Menu::Date { pos, view: (k.div_euclid(12), k.rem_euclid(12) as u32 + 1) });
                        cx.notify();
                    }),
                )
        };
        let header = div()
            .pt(px(12.))
            .pr(px(12.))
            .pb(px(8.))
            .pl(px(16.))
            .flex()
            .items_center()
            .justify_between()
            .child(div().text_size(px(13.)).line_height(px(22.)).font_weight(FontWeight::SEMIBOLD).child(crate::localized_format!("{vy}年 {vm}月", "{vy}-{vm:02}")))
            .child(div().flex().gap(px(4.)).child(nav("meta-date-prev", "chevron-left", -1)).child(nav("meta-date-next", "chevron-right", 1)));
        let mut weeks = div().mb(px(4.)).flex().justify_between();
        for w in [crate::settings::text("一"), crate::settings::text("二"), crate::settings::text("三"), crate::settings::text("四"), crate::settings::text("五"), crate::settings::text("六"), crate::settings::text("日")] {
            weeks = weeks.child(div().w(px(28.)).h(px(28.)).flex().items_center().justify_center().text_size(px(12.)).text_color(Theme::text_placeholder()).child(w));
        }
        let mut body = div().flex().flex_col().gap(px(2.)).child(weeks);
        for r in 0..6 {
            let mut row = div().flex().justify_between();
            for c in 0..7 {
                let day = start + r * 7 + c;
                let (y, m, dd) = super::countdown::from_days(day);
                let is_set = Some((y, m, dd)) == set;
                let today = (y, m, dd) == (ty, tm, td);
                let is_created = set.is_none() && (y, m, dd) == created;
                let inside = (y, m) == (vy, vm);
                row = row.child(
                    div()
                        .id(("meta-date-day", day as usize))
                        .size(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(8.))
                        .text_size(px(13.))
                        .cursor_pointer()
                        .text_color(if is_set {
                            gpui::white()
                        } else if today || is_created {
                            Theme::primary()
                        } else if inside {
                            Theme::text()
                        } else {
                            Theme::text_placeholder()
                        })
                        .when(is_set, |d| d.bg(Theme::primary()).font_weight(FontWeight::SEMIBOLD))
                        .when((today || is_created) && !is_set, |d| d.font_weight(FontWeight::SEMIBOLD))
                        .when(!is_set, |d| d.hover(|s| s.bg(Theme::fill_hover())))
                        .child(dd.to_string())
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |e, _, window, cx| {
                                e.set_date(y, m, dd, cx);
                                e.close_menu_focus(window, cx);
                            }),
                        ),
                );
            }
            body = body.child(row);
        }
        let el = menu().id("date-menu").w(px(244.)).child(header).child(div().px(px(12.)).pb(px(12.)).child(body));
        Some(at(pos, Corner::TopLeft, el))
    }

    // ---- status popover

    pub(super) fn open_status_menu(&mut self, pos: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.menu, Some(Menu::Status { .. })) {
            return self.close_menu_focus(window, cx);
        }
        let current = self.core.doc.meta.status.clone();
        let custom = current.filter(|s| !STATUS_PRESETS.contains(&s.as_str())).unwrap_or_default();
        self.status_field.update(cx, |f, cx| f.set_text(&custom, cx));
        self.menu = Some(Menu::Status { pos: self.rel(pos) });
        let fh = self.status_field.read(cx).focus_handle(cx);
        window.focus(&fh);
        cx.notify();
    }

    pub(super) fn on_status_field(&mut self, ev: &TextFieldEvent, window: &mut Window, cx: &mut Context<Self>) {
        match ev {
            TextFieldEvent::Submit => {
                let v = self.status_field.read(cx).text.trim().to_string();
                if !v.is_empty() {
                    self.commit_status(Some(v), window, cx);
                }
            }
            TextFieldEvent::Cancel => self.close_menu_focus(window, cx),
            _ => {}
        }
    }

    pub(super) fn status_menu(&self, pos: Point<Pixels>, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let current = self.core.doc.meta.status.clone();
        let mut panel = menu().id("status-menu").w(px(180.)).py(px(MENU_PAD_Y)).on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        for (i, s) in STATUS_PRESETS.into_iter().enumerate() {
            let on = current.as_deref() == Some(s);
            panel = panel.child(
                menu_row(("status-preset", i), "status-row".into())
                    .justify_between()
                    .when(on, |d| d.bg(Theme::fill_hover()))
                    .child(s)
                    .when(on, |d| d.child(icon("check", 14., Theme::primary())))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |e, _, window, cx| {
                            cx.stop_propagation();
                            e.commit_status(Some(s.to_string()), window, cx);
                        }),
                    ),
            );
        }
        if current.is_some() {
            panel = panel.child(
                menu_row("status-clear", "status-row".into()).text_color(Theme::text_caption()).child(crate::settings::text("移除")).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|e, _, window, cx| {
                        cx.stop_propagation();
                        e.commit_status(None, window, cx);
                    }),
                ),
            );
        }
        let focused = self.status_field.read(cx).is_focused(window);
        panel = panel.child(
            div()
                .mx(px(MENU_INSET))
                .mt(px(4.))
                .h(px(30.))
                .px(px(8.))
                .flex()
                .items_center()
                .border_1()
                .rounded(px(ROW_RADIUS))
                .border_color(if focused { Theme::primary() } else { Theme::line_border() })
                .child(self.status_field.clone()),
        );
        Some(at(pos, Corner::TopLeft, panel))
    }

    // ---- tags: an inline search-or-create field in the bar itself, with
    // its matches in a dropdown below (not a boxed popover around both).

    pub(super) fn open_tags_menu(&mut self, pos: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.menu, Some(Menu::Tags { .. })) {
            return self.close_menu_focus(window, cx);
        }
        self.tag_field.update(cx, |f, cx| f.set_text("", cx));
        // Nudged below the chip row, since the input itself now lives there
        // too (`tag_input_el`) — only the suggestions dropdown anchors here.
        self.menu = Some(Menu::Tags { pos: self.rel(pos + point(px(0.), px(CHIP_H + 6.))), selected: 0 });
        let fh = self.tag_field.read(cx).focus_handle(cx);
        window.focus(&fh);
        cx.notify();
    }

    /// Existing tags matching the query, not already on this document, then
    /// whether a trailing "create new tag" row applies (a non-empty query
    /// that isn't already one of this document's or the vault's tags).
    fn tag_candidates(&self, cx: &App) -> (Vec<String>, bool) {
        let q = self.tag_field.read(cx).text.trim().to_lowercase();
        let added = &self.core.doc.meta.tags;
        let filtered: Vec<String> = self.tag_options.iter()
            .filter(|t| !added.iter().any(|a| a.eq_ignore_ascii_case(t)))
            .filter(|t| q.is_empty() || t.to_lowercase().contains(&q))
            .cloned()
            .collect();
        let exact = !q.is_empty() && (self.tag_options.iter().any(|t| t.to_lowercase() == q) || added.iter().any(|t| t.to_lowercase() == q));
        (filtered, !q.is_empty() && !exact)
    }

    fn pick_tag_candidate(&mut self, tag: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.add_tag(tag, cx);
        self.tag_field.update(cx, |f, cx| f.set_text("", cx));
        if let Some(Menu::Tags { pos, .. }) = self.menu {
            self.menu = Some(Menu::Tags { pos, selected: 0 });
        }
        let fh = self.tag_field.read(cx).focus_handle(cx);
        window.focus(&fh);
        cx.notify();
    }

    pub(super) fn on_tag_field(&mut self, ev: &TextFieldEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Menu::Tags { pos, selected }) = self.menu else { return };
        let (filtered, create) = self.tag_candidates(cx);
        let n = filtered.len() + create as usize;
        let step = |d: isize| (selected as isize + d).rem_euclid(n.max(1) as isize) as usize;
        match ev {
            TextFieldEvent::Changed => self.menu = Some(Menu::Tags { pos, selected: 0 }),
            TextFieldEvent::Up if n > 0 => self.menu = Some(Menu::Tags { pos, selected: step(-1) }),
            TextFieldEvent::Down if n > 0 => self.menu = Some(Menu::Tags { pos, selected: step(1) }),
            TextFieldEvent::Submit => {
                let tag = if selected < filtered.len() { filtered.get(selected).cloned() } else if create { Some(self.tag_field.read(cx).text.clone()) } else { None };
                if let Some(tag) = tag {
                    self.pick_tag_candidate(&tag, window, cx);
                }
                return;
            }
            TextFieldEvent::Cancel => return self.close_menu_focus(window, cx),
            _ => return,
        }
        cx.notify();
    }

    /// Just the suggestions dropdown — the search field itself now lives
    /// inline in the bar (`tag_input_el`), not boxed together with this.
    pub(super) fn tags_menu(&self, pos: Point<Pixels>, selected: usize, _window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (filtered, create) = self.tag_candidates(cx);
        let query = self.tag_field.read(cx).text.trim().to_string();
        let mut list = div().id("tags-list").max_h(px(208.)).overflow_y_scroll().flex().flex_col();
        for (i, t) in filtered.iter().enumerate() {
            let t = t.clone();
            list = list.child(
                menu_row(("tag-option", i), "tag-row".into())
                    .when(i == selected, |d| d.bg(Theme::fill_hover()))
                    .child(t.clone())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |e, _, window, cx| {
                            cx.stop_propagation();
                            e.pick_tag_candidate(&t, window, cx);
                        }),
                    ),
            );
        }
        if create {
            let i = filtered.len();
            let t = query.clone();
            list = list.child(
                menu_row("tag-create", "tag-row".into())
                    .when(i == selected, |d| d.bg(Theme::fill_hover()))
                    .child(icon("plus", 13., Theme::icon()))
                    .child(div().flex_1().min_w_0().truncate().child(t.clone()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |e, _, window, cx| {
                            cx.stop_propagation();
                            e.pick_tag_candidate(&t, window, cx);
                        }),
                    ),
            );
        }
        if filtered.is_empty() && !create {
            list = list.child(div().h(px(MENU_ROW_H)).px(px(MENU_INSET + 8.)).flex().items_center().text_size(px(13.)).text_color(Theme::text_caption()).child(crate::settings::text("无匹配结果")));
        }
        let el = menu().id("tags-menu").occlude().w(px(180.)).py(px(MENU_PAD_Y)).on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(list);
        Some(at(pos, Corner::TopLeft, el))
    }

    // ---- the bar itself

    /// Whether the open document even has a front-matter bar to show
    /// (Markdown only — DocxXML documents have no front matter).
    pub(super) fn has_meta_bar(&self) -> bool {
        self.markdown
    }

    pub(super) fn meta_bar_el(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let tags = self.core.doc.meta.tags.clone();
        let status = self.core.doc.meta.status.clone();
        let mut bar = div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap(px(6.))
            .mt(px(14.))
            .mb(px(8.))
            .text_size(px(12.))
            .line_height(px(18.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(self.date_chip_el(cx))
            .child(self.status_chip_el(status, cx));
        for t in tags {
            bar = bar.child(self.tag_chip_el(t, cx));
        }
        if matches!(self.menu, Some(Menu::Tags { .. })) {
            bar.child(self.tag_input_el(window, cx)).into_any_element()
        } else {
            bar.child(self.add_tag_button_el(cx)).into_any_element()
        }
    }

    fn date_chip_el(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let open = matches!(self.menu, Some(Menu::Date { .. }));
        let (set, created) = self.display_date();
        let (y, m, d) = set.unwrap_or(created);
        div()
            .id("meta-date")
            .flex_none()
            .h(px(CHIP_H))
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(4.))
            .rounded(px(CHIP_H / 2.))
            .when(open, |d| d.bg(Theme::fill_pressed()))
            .text_color(if set.is_some() { Theme::text() } else { Theme::text_caption() })
            .child(icon("calendar", 13., Theme::icon()))
            .child(format!("{y:04}-{m:02}-{d:02}"))
            .when(!self.read_only, |el| {
                el.cursor_pointer().hover(|s| s.bg(Theme::fill_hover())).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|e, ev: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        e.open_date_menu(ev.position, window, cx);
                    }),
                )
            })
    }

    fn status_chip_el(&self, status: Option<String>, cx: &mut Context<Self>) -> impl IntoElement {
        let open = matches!(self.menu, Some(Menu::Status { .. }));
        let (bg, color, label) = match &status {
            Some(s) => {
                let (bg, color) = chip_colors(hue_of(s));
                (bg, color, s.clone())
            }
            None => (Theme::fill_hover(), Theme::icon(), String::new()),
        };
        div()
            .id("meta-status")
            .flex_none()
            .h(px(CHIP_H))
            .px(px(if status.is_some() { 8. } else { 4. }))
            .flex()
            .items_center()
            .rounded(px(CHIP_H / 2.))
            .bg(bg)
            .text_color(color)
            .when(open, |d| d.bg(Theme::fill_pressed()))
            .when(status.is_none(), |d| d.child(icon("plus", 12., Theme::icon())))
            .when(status.is_some(), |d| d.child(label))
            .when(!self.read_only, |d| {
                d.cursor_pointer().on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|e, ev: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        e.open_status_menu(ev.position, window, cx);
                    }),
                )
            })
    }

    fn tag_chip_el(&self, tag: String, cx: &mut Context<Self>) -> impl IntoElement {
        let (bg, color) = chip_colors(hue_of(&tag));
        div()
            .id(("meta-tag", hash_str(&tag) as usize))
            .flex_none()
            .h(px(CHIP_H))
            .pl(px(8.))
            .pr(px(4.))
            .flex()
            .items_center()
            .gap(px(2.))
            .rounded(px(CHIP_H / 2.))
            .bg(bg)
            .text_color(color)
            .child(tag.clone())
            .when(!self.read_only, |d| {
                d.child(
                    div()
                        .id(("meta-tag-remove", hash_str(&tag) as usize))
                        .size(px(14.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(7.))
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::ta(255, 255, 255, 0.25)))
                        .child(icon("x", 10., color))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |e, _, _, cx| {
                                cx.stop_propagation();
                                e.remove_tag(&tag, cx);
                            }),
                        ),
                )
            })
    }

    fn add_tag_button_el(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("meta-add-tag")
            .flex_none()
            .size(px(CHIP_H))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(CHIP_H / 2.))
            .text_color(Theme::icon())
            .hover(|s| s.bg(Theme::fill_hover()))
            .child(icon("plus", 12., Theme::icon()))
            .tip(crate::settings::text("添加标签"), None)
            .when(!self.read_only, |d| {
                d.cursor_pointer().on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|e, ev: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        e.open_tags_menu(ev.position, window, cx);
                    }),
                )
            })
    }

    /// The tag search-or-create field, inline in the bar right after the
    /// chips (not inside the suggestions dropdown — see `tags_menu`).
    fn tag_input_el(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.tag_field.read(cx).is_focused(window);
        div()
            .id("meta-tag-input")
            .flex_none()
            .h(px(CHIP_H))
            .w(px(120.))
            .px(px(8.))
            .flex()
            .items_center()
            .rounded(px(CHIP_H / 2.))
            .border_1()
            .border_color(if focused { Theme::primary() } else { Theme::line_border() })
            .child(self.tag_field.clone())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    }
}
