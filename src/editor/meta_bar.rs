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

fn chip_colors(hue: &str) -> (Hsla, Hsla) {
    (theme::bg_color(&format!("light-{hue}")).unwrap_or(Theme::fill_hover()), theme::text_color(hue).unwrap_or(Theme::text()))
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

    fn set_date(&mut self, date: &str, cx: &mut Context<Self>) {
        let date = date.trim().to_string();
        self.set_meta(|m| m.date = (!date.is_empty()).then_some(date), cx);
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

    /// Every note's tags just opened this document; a vault edited while it
    /// stays open won't refresh the list until it's reopened.
    pub fn sync_date_placeholder(&mut self, cx: &mut Context<Self>) {
        let (y, mo, d, _, _) = crate::time::civil(self.created);
        let hint = format!("{y:04}-{mo:02}-{d:02}");
        self.date_field.update(cx, |f, cx| f.set_placeholder(hint, cx));
    }

    // ---- date field (no popover: a plain inline text field)

    pub(super) fn on_date_field(&mut self, ev: &TextFieldEvent, window: &mut Window, cx: &mut Context<Self>) {
        match ev {
            TextFieldEvent::Submit | TextFieldEvent::Cancel => {
                if *ev == TextFieldEvent::Submit {
                    let v = self.date_field.read(cx).text.clone();
                    self.set_date(&v, cx);
                }
                let v = self.core.doc.meta.date.clone().unwrap_or_default();
                self.date_field.update(cx, |f, cx| f.set_text(&v, cx));
                window.focus(&self.focus);
            }
            _ => {}
        }
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

    // ---- tags popover: search existing tags or create a new one

    pub(super) fn open_tags_menu(&mut self, pos: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.menu, Some(Menu::Tags { .. })) {
            return self.close_menu_focus(window, cx);
        }
        self.tag_field.update(cx, |f, cx| f.set_text("", cx));
        self.menu = Some(Menu::Tags { pos: self.rel(pos), selected: 0 });
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
                    self.add_tag(&tag, cx);
                    self.tag_field.update(cx, |f, cx| f.set_text("", cx));
                    self.menu = Some(Menu::Tags { pos, selected: 0 });
                }
                return;
            }
            TextFieldEvent::Cancel => return self.close_menu_focus(window, cx),
            _ => return,
        }
        cx.notify();
    }

    pub(super) fn tags_menu(&self, pos: Point<Pixels>, selected: usize, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
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
                            e.add_tag(&t, cx);
                            e.tag_field.update(cx, |f, cx| f.set_text("", cx));
                            if let Some(Menu::Tags { pos, .. }) = e.menu {
                                e.menu = Some(Menu::Tags { pos, selected: 0 });
                            }
                            let fh = e.tag_field.read(cx).focus_handle(cx);
                            window.focus(&fh);
                            cx.notify();
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
                            e.add_tag(&t, cx);
                            e.tag_field.update(cx, |f, cx| f.set_text("", cx));
                            if let Some(Menu::Tags { pos, .. }) = e.menu {
                                e.menu = Some(Menu::Tags { pos, selected: 0 });
                            }
                            let fh = e.tag_field.read(cx).focus_handle(cx);
                            window.focus(&fh);
                            cx.notify();
                        }),
                    ),
            );
        }
        if filtered.is_empty() && !create {
            list = list.child(div().h(px(MENU_ROW_H)).px(px(MENU_INSET + 8.)).flex().items_center().text_size(px(13.)).text_color(Theme::text_caption()).child(crate::settings::text("无匹配结果")));
        }
        let focused = self.tag_field.read(cx).is_focused(window);
        let el = menu()
            .id("tags-menu")
            .occlude()
            .w(px(220.))
            .py(px(MENU_PAD_Y))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .mx(px(MENU_INSET))
                    .mb(px(6.))
                    .h(px(32.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(if focused { Theme::primary() } else { Theme::line_border() })
                    .child(icon("search", 14., Theme::icon()))
                    .child(self.tag_field.clone()),
            )
            .child(list);
        Some(at(pos, Corner::TopLeft, el))
    }

    // ---- the bar itself

    /// Whether the open document even has a front-matter bar to show
    /// (Markdown only — DocxXML documents have no front matter).
    pub(super) fn has_meta_bar(&self) -> bool {
        self.markdown
    }

    pub(super) fn meta_bar_el(&self, cx: &mut Context<Self>) -> AnyElement {
        let tags = self.core.doc.meta.tags.clone();
        let status = self.core.doc.meta.status.clone();
        let mut bar = div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap(px(6.))
            .mt(px(2.))
            .mb(px(8.))
            .text_size(px(12.))
            .line_height(px(18.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(self.date_chip_el())
            .child(self.status_chip_el(status, cx));
        for t in tags {
            bar = bar.child(self.tag_chip_el(t, cx));
        }
        bar.child(self.add_tag_button_el(cx)).into_any_element()
    }

    fn date_chip_el(&self) -> impl IntoElement {
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .text_color(Theme::text_caption())
            .child(icon("calendar", 13., Theme::icon()))
            .child(div().w(px(84.)).child(self.date_field.clone()))
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
                        .hover(|s| s.bg(theme::ta(0, 0, 0, 0.08)))
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
        let open = matches!(self.menu, Some(Menu::Tags { .. }));
        div()
            .id("meta-add-tag")
            .flex_none()
            .size(px(CHIP_H))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(CHIP_H / 2.))
            .text_color(Theme::icon())
            .when(open, |d| d.bg(Theme::fill_pressed()))
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
}
