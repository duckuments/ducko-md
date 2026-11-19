//! Left pane: document list with search. Rows, sections and footer, sunken:
//! no panel, border or shadow, straight on the window surface (only the note
//! floats). 30pt rows on a 31pt pitch, selection pill radius 9.

use crate::app::{Confirm, NavFocus, RowMenu, View, Workspace};
use crate::settings::display_title;
use crate::store::Entry;
use crate::theme::{self, Theme, craft};
use crate::ui::{TipExt, TitleBarExt, icon, popover, ui_text};
use gpui::{prelude::FluentBuilder, *};

/// Row "…" menu geometry: 32pt rows, 11pt dividers, 6pt padding inside
/// a 1pt border.
const ROW_MENU_W: f32 = 180.;
const ROW_ITEM: f32 = crate::ui::MENU_ROW_H;
const ROW_DIVIDER: f32 = 11.;
const ROW_ITEMS: usize = 9;
const ROW_DIVIDERS: usize = 2;
const DOWNLOAD_INDEX: usize = 5;
const DIVIDERS_BEFORE_DOWNLOAD: usize = 1;

const fn row_menu_height(items: usize, dividers: usize) -> f32 {
    2. + crate::ui::MENU_PAD_Y * 2. + items as f32 * ROW_ITEM + dividers as f32 * ROW_DIVIDER
}

const fn row_sub_top(menu_top: f32) -> f32 {
    menu_top + DOWNLOAD_INDEX as f32 * ROW_ITEM + DIVIDERS_BEFORE_DOWNLOAD as f32 * ROW_DIVIDER
}

const _: () = assert!(row_menu_height(ROW_ITEMS, ROW_DIVIDERS) == 324.);
const _: () = assert!(row_sub_top(0.) == 171.);

/// Sidebar geometry (measured from the window edge), shared by the
/// document sidebar and 设置's: selection pills start 14pt in, every row is
/// 30pt with radius 9 on a 31pt pitch, glyphs (18pt) centre at 37pt and
/// labels start at 54pt; groups are 16pt apart, a section header is 34pt
/// (6 above its 28pt label).
pub(crate) const SB_X: f32 = 14.;
pub(crate) const ROW_H: f32 = 30.;
pub(crate) const ROW_R: f32 = 9.;
pub(crate) const ROW_GAP: f32 = 1.;
pub(crate) const ROW_ICON: f32 = 18.;
pub(crate) const GROUP_GAP: f32 = 16.;
pub(crate) const SECTION_H: f32 = 34.;
pub(crate) const SECTION_TOP: f32 = 6.;

/// The note nav (the middle column, between the folder nav and the editor):
/// a fixed width, unlike the folder nav which the user can drag.
pub(crate) const NOTE_NAV_W: f32 = 240.;

/// 批量操作 caps at 50 pages.
pub(crate) const BATCH_LIMIT: usize = 50;

/// The footer row: like 主页 (same pill, glyph and label columns) under
/// the footer line, so the bottom corner mirrors the top.
pub(crate) fn footer_row(id: &'static str, glyph: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(ROW_H))
        .pl(px(6.))
        .pr(px(4.))
        .flex()
        .items_center()
        .rounded(px(ROW_R))
        .cursor_pointer()
        .text_size(px(theme::ui_font()))
        .line_height(px(20.))
        .text_color(Theme::sidebar_text())
        .hover(|s| s.bg(Theme::sidebar_hover()))
        .child(div().size(px(20.)).flex_none().flex().items_center().justify_center().mr(px(9.)).child(icon(glyph, ROW_ICON, Theme::icon())))
        .child(label)
}

/// The footer's divider and padding around `row`.
pub(crate) fn footer_slot(row: impl IntoElement) -> Div {
    div().pt(px(6.)).pb(px(10.)).px(px(SB_X)).child(row)
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Check {
    Off,
    Partial,
    On,
}

/// 14px checkbox; an unchecked box turns blue when `group` is hovered.
pub(crate) fn checkbox(state: Check, disabled: bool, group: &'static str) -> Div {
    let white = Theme::bg_body();
    div()
        .size(px(14.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .when(state != Check::Off, |d| d.bg(Theme::primary()))
        .when(state == Check::Off, |d| {
            d.border_1()
                .when(disabled, |d| d.border_color(Theme::line_border()).bg(theme::c(craft::N100)))
                .when(!disabled, |d| d.border_color(theme::c(craft::N500)).bg(white).group_hover(group, |s| s.border_color(Theme::primary())))
        })
        .when(state == Check::On, |d| d.child(icon("todo-check", 12., white)))
        .when(state == Check::Partial, |d| d.child(div().w(px(8.)).h(px(2.)).rounded(px(1.)).bg(white)))
}

impl Workspace {
    /// Sidebar width for the current window: the dragged width, kept between
    /// 268 and min(800, window − 540).
    pub(crate) fn sidebar_w(&self, window: &Window) -> f32 {
        let vw = f32::from(window.viewport_size().width);
        let max = theme::SIDEBAR_MAX.min(vw - theme::SIDEBAR_MIN_CONTENT).max(theme::SIDEBAR_MIN);
        self.sidebar_width.clamp(theme::SIDEBAR_MIN, max)
    }

    /// Note nav width for the current window: dragged, kept between
    /// `NOTE_NAV_MIN` and `NOTE_NAV_MAX`, further capped so the folder column
    /// and the editor's own minimum still fit.
    pub(crate) fn note_nav_w(&self, window: &Window) -> f32 {
        let vw = f32::from(window.viewport_size().width);
        let max = theme::NOTE_NAV_MAX.min(vw - self.sidebar_w(window) - crate::app::DOC_MIN_W).max(theme::NOTE_NAV_MIN);
        self.note_nav_width.clamp(theme::NOTE_NAV_MIN, max)
    }

    /// Total width of the nav: the folder column alone in 设置/回收站 (which
    /// keep their own single pane), folder + note columns everywhere else.
    pub(crate) fn nav_w(&self, window: &Window) -> f32 {
        if !self.sidebar_open {
            return 0.;
        }
        let w = self.sidebar_w(window);
        if matches!(self.view, View::Settings | View::Trash) { w } else { w + self.note_nav_w(window) }
    }

    /// The left column: folders only, no docs. Picking one (or none, via 主页)
    /// drives what the note column beside it shows.
    pub(crate) fn render_folder_nav(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let header = div()
            .relative()
            .h(px(crate::app::HEADER_H))
            .flex_none()
            .flex()
            .items_center()
            .pl(px(theme::TRAFFIC_LIGHTS_W))
            .pr(px(12.))
            .title_bar()
            .when(self.nav_focus == NavFocus::Folder, |d| d.child(div().absolute().bottom_0().left_0().right_0().h(px(2.)).bg(Theme::primary())))
            .child(
                div()
                    .id("collapse-sidebar")
                    .no_drag()
                    .flex_none()
                    .ml(px(4.))
                    .size(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::sidebar_hover()))
                    .tip(crate::settings::tr("收起侧栏", "Hide sidebar"), Some(crate::shortcuts::hint("workspace::ToggleSidebar", "⌘ + ]")))
                    .child(icon("panel-left-close", 17., Theme::icon()))
                    .on_click(cx.listener(|w, _, _, cx| {
                        w.sidebar_open = false;
                        cx.notify();
                    })),
            )
            .child(div().flex_1())
            .children(crate::settings::update_pill(cx));

        // The nav is just 主页: 36px items, a blue tint when selected. Its
        // icon column (x 17) is shared with 搜索 and the doc rows.
        let home_on = self.view == View::Home;
        let nav = div().flex_none().pt(px(4.)).pb(px(4.)).px(px(SB_X)).child(
            div()
                .id("nav-home")
                .h(px(ROW_H))
                .pl(px(6.))
                .pr(px(4.))
                .flex()
                .items_center()
                .rounded(px(ROW_R))
                .cursor_pointer()
                .text_size(px(theme::ui_font()))
                .line_height(px(20.))
                .when(home_on, |d| d.bg(Theme::sidebar_selected()).text_color(Theme::text()))
                .when(!home_on, |d| d.text_color(Theme::sidebar_text()).hover(|s| s.bg(Theme::sidebar_hover())))
                .child(
                    // Selected or not, the same outline: the row's fill marks the selection.
                    div().size(px(20.)).flex_none().flex().items_center().justify_center().mr(px(9.)).child(icon(
                        "home",
                        ROW_ICON,
                        Theme::icon(),
                    )),
                )
                .child(crate::settings::tr("主页", "Home"))
                .on_click(cx.listener(|w, _, _, cx| {
                    w.folder_current = None;
                    w.show_view(View::Home, cx);
                })),
        );

        let tree = self.folder_tree_rows(None, 0, cx);
        let footer = div().flex_none().flex().flex_col().child(footer_slot(
            footer_row("ft-settings", "settings-gear", crate::settings::tr("设置", "Settings"))
                .on_click(cx.listener(|w, _, _, cx| w.show_view(View::Settings, cx))),
        ));

        div()
            .w(px(self.sidebar_w(window)))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .child(header)
            .child(nav)
            .child(
                div()
                    .id("folder-nav-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.list_scroll)
                    .child(div().flex().flex_col().px(px(SB_X)).pb(px(8.)).pt(px(4.)).child(tree)),
            )
            .children(crate::changelog::card(cx.listener(|_, _, _, cx| cx.notify())))
            .child(footer)
            .into_any_element()
    }

    /// 文件夹 only, recursively; a folder's own contents show in the note
    /// column once it is picked, not inline here.
    fn folder_tree_rows(&self, parent: Option<&str>, depth: usize, cx: &mut Context<Self>) -> Div {
        let mut col = div().flex().flex_col();
        let folders: Vec<Entry> = self.level_entries(parent).into_iter().filter(|e| e.meta.folder.is_some()).collect();
        for f in &folders {
            let selected = self.folder_current.as_deref() == Some(f.id.as_str());
            col = col.child(self.nav_row(f, depth, selected, cx));
            if self.expanded.contains(&f.id) {
                col = col.child(self.folder_tree_rows(Some(&f.id), depth + 1, cx));
            }
        }
        col
    }

    /// The note column: the picked folder's notes (置顶 and 本机文件 at the
    /// top level, as they were in 我的文档), or the open document's siblings
    /// when nothing was picked explicitly (open() keeps `folder_current` in
    /// step with it).
    pub(crate) fn render_note_nav(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let folder_id = self.folder_current.clone();
        let title = folder_id
            .as_deref()
            .and_then(|id| self.entries.iter().find(|e| e.id == id))
            .map(|e| display_title(&e.title))
            .unwrap_or_else(|| crate::settings::tr("我的文档", "My documents").to_string());

        let header = div()
            .relative()
            .h(px(crate::app::HEADER_H))
            .flex_none()
            .flex()
            .items_center()
            .px(px(SB_X))
            .title_bar()
            .when(self.nav_focus == NavFocus::Note, |d| d.child(div().absolute().bottom_0().left_0().right_0().h(px(2.)).bg(Theme::primary())))
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(Theme::text())
                    .child(ui_text(title)),
            );

        let highlighted = |e: &Entry| -> bool {
            (self.view == View::Doc && self.current.as_deref() == Some(e.id.as_str()))
                || (self.nav_focus == NavFocus::Note && self.note_highlight.as_deref() == Some(e.id.as_str()))
        };
        let mut body = div().flex().flex_col().px(px(SB_X)).pb(px(8.)).pt(px(4.));
        if folder_id.is_none() {
            // Pinned docs are shortcuts: they stay in the plain list too.
            let pinned: Vec<Entry> = self.entries.iter().filter(|e| e.meta.pinned).cloned().collect();
            if !pinned.is_empty() {
                body = body.child(self.nav_label(crate::settings::text("置顶")));
                for e in &pinned {
                    body = body.child(self.note_card(e, highlighted(e), cx));
                }
                body = body.child(div().h(px(GROUP_GAP)));
            }
            // 本机文件: Markdown files edited where they are, outside the library.
            let local: Vec<Entry> = self.local_entries().into_iter().cloned().collect();
            if !local.is_empty() {
                body = body.child(self.nav_label(crate::settings::tr("本机文件", "Files on this computer")));
                for e in &local {
                    body = body.child(self.note_card(e, highlighted(e), cx));
                }
                body = body.child(div().h(px(GROUP_GAP)));
            }
        }
        let notes: Vec<Entry> = self.level_entries(folder_id.as_deref()).into_iter().filter(|e| e.meta.folder.is_none() && !e.meta.local()).collect();
        if notes.is_empty() {
            if folder_id.is_some() {
                body = body.child(crate::trash::empty_state("illus/empty-list.svg", crate::settings::tr("这个文件夹还是空的", "This folder is empty")));
            }
        } else {
            for e in &notes {
                body = body.child(self.note_card(e, highlighted(e), cx));
            }
        }

        div()
            .w(px(self.note_nav_w(window)))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .child(header)
            .child(div().id("note-nav-list").flex_1().min_h_0().overflow_y_scroll().child(body))
            .into_any_element()
    }

    /// A plain section label (no fold, no batch toolbar — those stayed with
    /// the retired combined sidebar).
    fn nav_label(&self, title: &str) -> Div {
        div().h(px(SECTION_H)).pl(px(4.)).pt(px(SECTION_TOP)).flex().items_center().child(
            div()
                .px(px(4.))
                .text_size(px(theme::ui_font()))
                .line_height(px(theme::ui_font() + 7.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Theme::sidebar_section())
                .child(title.to_string()),
        )
    }

    /// Picking a folder in the nav: it does not touch the open document, only
    /// what the note column shows.
    pub(crate) fn select_folder(&mut self, id: &str) {
        self.folder_current = Some(id.to_string());
    }

    /// Folder ids in the order the left nav draws them (pre-order, respecting
    /// `self.expanded`) — the same traversal `folder_tree_rows` renders, kept
    /// separate so arrow-key order always matches what's on screen.
    fn visible_folder_ids(&self, parent: Option<&str>, out: &mut Vec<String>) {
        let folders: Vec<Entry> = self.level_entries(parent).into_iter().filter(|e| e.meta.folder.is_some()).collect();
        for f in &folders {
            out.push(f.id.clone());
            if self.expanded.contains(&f.id) {
                self.visible_folder_ids(Some(&f.id), out);
            }
        }
    }

    /// Ids in the order the note nav draws them: 置顶 then 本机文件 (at the
    /// top level only), then the selected folder's notes.
    pub(crate) fn visible_notes(&self) -> Vec<String> {
        let folder_id = self.folder_current.clone();
        let mut out = vec![];
        if folder_id.is_none() {
            out.extend(self.entries.iter().filter(|e| e.meta.pinned).map(|e| e.id.clone()));
            out.extend(self.local_entries().into_iter().map(|e| e.id.clone()));
        }
        out.extend(self.level_entries(folder_id.as_deref()).into_iter().filter(|e| e.meta.folder.is_none() && !e.meta.local()).map(|e| e.id.clone()));
        out
    }

    /// cmd-1/2: jump keyboard focus to a nav column, opening the sidebar if
    /// it was hidden.
    pub(crate) fn focus_nav(&mut self, to: NavFocus, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_open = true;
        self.nav_focus = to;
        if to == NavFocus::Note && self.note_highlight.is_none() {
            self.note_highlight = self.current.clone().or_else(|| self.visible_notes().into_iter().next());
        }
        window.focus(&self.focus);
        cx.notify();
    }

    /// Up/Down: move the highlighted row in whichever column has nav focus.
    pub(crate) fn nav_move(&mut self, delta: i32, cx: &mut Context<Self>) {
        let (ids, current) = match self.nav_focus {
            NavFocus::Folder => {
                let mut ids = vec![];
                self.visible_folder_ids(None, &mut ids);
                (ids, self.folder_current.clone())
            }
            NavFocus::Note => (self.visible_notes(), self.note_highlight.clone().or_else(|| self.current.clone())),
        };
        if ids.is_empty() {
            return;
        }
        let next = match current.and_then(|c| ids.iter().position(|x| *x == c)) {
            Some(i) => (i as i32 + delta).clamp(0, ids.len() as i32 - 1) as usize,
            None if delta > 0 => 0,
            None => ids.len() - 1,
        };
        match self.nav_focus {
            NavFocus::Folder => self.folder_current = Some(ids[next].clone()),
            NavFocus::Note => self.note_highlight = Some(ids[next].clone()),
        }
        cx.notify();
    }

    /// Left/Right: hop between the folder and note columns.
    pub(crate) fn nav_switch(&mut self, to: NavFocus, cx: &mut Context<Self>) {
        self.nav_focus = to;
        if to == NavFocus::Note && self.note_highlight.is_none() {
            self.note_highlight = self.current.clone().or_else(|| self.visible_notes().into_iter().next());
        }
        cx.notify();
    }

    /// Enter: toggle the highlighted folder's expand state, or open/select
    /// the highlighted note.
    pub(crate) fn nav_activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.nav_focus {
            NavFocus::Folder => {
                if let Some(id) = self.folder_current.clone()
                    && !self.expanded.remove(&id) {
                        self.expanded.insert(id);
                }
            }
            NavFocus::Note => {
                if let Some(id) = self.note_highlight.clone() {
                    let folder = self.entries.iter().any(|e| e.id == id && e.meta.folder.is_some());
                    if folder { self.select_folder(&id); } else { self.open(&id, window, cx); }
                }
            }
        }
        cx.notify();
    }

    /// A folder row in the left nav: an icon, a disclosure chevron when it
    /// has subfolders, and the usual rename/"…" menu parity. No drag — the
    /// old combined sidebar's reorder/drop system stayed behind.
    fn nav_row(&self, e: &Entry, depth: usize, selected: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let (id_click, id_menu, id_more, id_toggle) = (e.id.clone(), e.id.clone(), e.id.clone(), e.id.clone());
        let has_children = self.entries.iter().any(|c| c.meta.folder.is_some() && c.meta.parent.as_deref() == Some(e.id.as_str()));
        let expanded = self.expanded.contains(&e.id);
        let renaming_field = self.rename.as_ref().filter(|r| r.id == e.id).map(|r| r.field.clone());

        let title = match renaming_field {
            Some(field) => div()
                .ml(px(2.))
                .mr(px(4.))
                .flex_1()
                .min_w_0()
                .h(px(26.))
                .px(px(6.))
                .flex()
                .items_center()
                .rounded(px(ROW_R - 2.))
                .bg(crate::theme::raised(Theme::bg_body()))
                .border_1()
                .border_color(Theme::sidebar_field_edge())
                .text_size(px(theme::ui_font()))
                .child(field)
                .into_any_element(),
            None => div()
                .pl(px(9.))
                .flex_1()
                .min_w_0()
                .text_size(px(theme::ui_font()))
                .line_height(px(ROW_H))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_color(if selected { Theme::text() } else { Theme::sidebar_text() })
                .child(ui_text(display_title(&e.title)))
                .into_any_element(),
        };
        let renaming = self.rename.as_ref().is_some_and(|r| r.id == e.id);

        div()
            .id(SharedString::from(format!("nav-{}", e.id)))
            .group("nav-row")
            .h(px(ROW_H))
            .w_full()
            .flex()
            .items_center()
            .mb(px(ROW_GAP))
            .rounded(px(ROW_R))
            .pl(px(6. + 18. * depth as f32))
            .pr(px(4.))
            .cursor_pointer()
            .when(selected, |d| d.bg(Theme::sidebar_selected()))
            .when(!selected, |d| d.hover(|s| s.bg(Theme::sidebar_hover())))
            .child(
                // A disclosure triangle for folders that hold subfolders, next
                // to the folder glyph (which stays a plain click target).
                div().size(px(14.)).flex_none().flex().items_center().justify_center()
                    .when(has_children, |d| {
                        d.cursor_pointer().child(
                            icon("disclosure", 10., Theme::icon())
                                .with_transformation(Transformation::rotate(radians(if expanded { std::f32::consts::FRAC_PI_2 } else { 0. }))),
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |w, _: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                if !w.expanded.remove(&id_toggle) {
                                    w.expanded.insert(id_toggle.clone());
                                }
                                cx.notify();
                            }),
                        )
                    }),
            )
            .child(
                div().size(px(20.)).flex_none().flex().items_center().justify_center()
                    .child(icon(if expanded { "folder-open" } else { "folder" }, ROW_ICON, Theme::icon())),
            )
            .child(title)
            .when(!renaming, |d| d.child(self.nav_more_button(&id_more, cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |w, _: &MouseDownEvent, _, cx| {
                    if w.rename.as_ref().is_some_and(|r| r.id == id_click) {
                        return;
                    }
                    w.nav_focus = NavFocus::Folder;
                    w.select_folder(&id_click);
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |w, ev: &MouseDownEvent, _, cx| {
                    w.row_menu = Some(RowMenu { id: id_menu.clone(), pos: ev.position, corner: Corner::TopLeft, sub: false, batch: false });
                    cx.notify();
                }),
            )
    }

    /// A short plain-text excerpt for the note card: the body's non-empty
    /// lines joined, capped so a card never grows past a couple of lines.
    fn note_excerpt(body: &str) -> String {
        let joined: String = body.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
        let capped: String = joined.chars().take(96).collect();
        if joined.chars().count() > 96 { format!("{capped}…") } else { capped }
    }

    /// A note's card in the note nav: title, a short excerpt and its created
    /// date — no icon, per the brief. Folders can end up here too (置顶
    /// shortcuts), in which case the click picks the folder instead of
    /// opening it.
    fn note_card(&self, e: &Entry, selected: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let folder = e.meta.folder.is_some();
        let (id_click, id_menu, id_more) = (e.id.clone(), e.id.clone(), e.id.clone());
        let renaming_field = self.rename.as_ref().filter(|r| r.id == e.id).map(|r| r.field.clone());
        let renaming = renaming_field.is_some();

        let title = match renaming_field {
            Some(field) => div()
                .h(px(26.))
                .px(px(6.))
                .flex()
                .items_center()
                .rounded(px(ROW_R - 2.))
                .bg(crate::theme::raised(Theme::bg_body()))
                .border_1()
                .border_color(Theme::sidebar_field_edge())
                .text_size(px(theme::ui_font()))
                .child(field)
                .into_any_element(),
            None => div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(theme::ui_font()))
                .font_weight(FontWeight::BOLD)
                .text_color(if selected { Theme::text() } else { Theme::sidebar_text() })
                .child(ui_text(display_title(&e.title)))
                .into_any_element(),
        };
        let excerpt = Self::note_excerpt(&e.body);

        div()
            .id(SharedString::from(format!("note-card-{}", e.id)))
            .group("nav-row")
            .w_full()
            .flex()
            .flex_col()
            .gap(px(3.))
            .p(px(10.))
            .mb(px(6.))
            .rounded(px(ROW_R))
            .cursor_pointer()
            .when(selected, |d| d.bg(Theme::sidebar_selected()))
            .when(!selected, |d| d.hover(|s| s.bg(Theme::sidebar_hover())))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(div().flex_1().min_w_0().child(title))
                    .when(!renaming, |d| d.child(self.nav_more_button(&id_more, cx))),
            )
            .when(!excerpt.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(px(theme::ui_font_small()))
                        .line_height(px(theme::ui_font_small() + 5.))
                        .text_color(Theme::sidebar_text())
                        .overflow_hidden()
                        .child(excerpt),
                )
            })
            .child(
                div()
                    .text_size(px(theme::ui_font_small()))
                    .text_color(Theme::text_placeholder())
                    .child(crate::ui::format_date(e.meta.created)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |w, _: &MouseDownEvent, window, cx| {
                    if w.rename.as_ref().is_some_and(|r| r.id == id_click) {
                        return;
                    }
                    w.nav_focus = NavFocus::Note;
                    w.note_highlight = Some(id_click.clone());
                    if folder { w.select_folder(&id_click); } else { w.open(&id_click, window, cx); }
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |w, ev: &MouseDownEvent, _, cx| {
                    w.row_menu = Some(RowMenu { id: id_menu.clone(), pos: ev.position, corner: Corner::TopLeft, sub: false, batch: false });
                    cx.notify();
                }),
            )
    }

    /// The nav rows' "…": no hover-reveal animation (that lived in the
    /// combined sidebar's per-frame spring system), just a plain group-hover
    /// fade, opening the same `render_row_menu`.
    fn nav_more_button(&self, id: &str, cx: &mut Context<Self>) -> Stateful<Div> {
        let id = id.to_string();
        let (id_open, id_click) = (id.clone(), id);
        let open = self.row_menu.as_ref().is_some_and(|m| m.id == id_open);
        div()
            .id(SharedString::from(format!("nav-more-{id_open}")))
            .size(px(22.))
            .mr(px(4.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.))
            .when(!open, |d| d.opacity(0.).group_hover("nav-row", |s| s.opacity(1.)))
            .when(open, |d| d.bg(Theme::sidebar_selected()))
            .hover(|s| s.bg(Theme::sidebar_selected()))
            .child(icon("ellipsis", 15., Theme::icon()))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |w, ev: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    w.row_menu = Some(RowMenu { id: id_click.clone(), pos: ev.position, corner: Corner::TopLeft, sub: false, batch: false });
                    cx.notify();
                }),
            )
    }

    /// Width handle: a 15px hit strip straddling the sidebar edge (6px inside,
    /// 9px over the note's margin). Borderless like the sidebar: only the
    /// cursor changes, and a faint line shows while dragging.
    pub(crate) fn render_resize_handle(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let w = self.sidebar_w(window);
        let dragging = self.sidebar_drag;
        div()
            .id("sidebar-resize")
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(w - 6.))
            .w(px(15.))
            .group("sidebar-resize")
            .cursor(CursorStyle::ResizeLeftRight)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(5.))
                    .w(px(2.))
                    .rounded(px(1.))
                    .when(dragging, |d| d.bg(Theme::primary()))
                    .group_hover("sidebar-resize", |s| s.bg(Theme::primary())),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |ws, _, window, cx| {
                    cx.stop_propagation();
                    ws.sidebar_width = ws.sidebar_w(window);
                    ws.sidebar_drag = true;
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    /// While dragging, a transparent full-window layer keeps the resize cursor
    /// and follows the mouse.
    pub(crate) fn render_resize_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.sidebar_drag {
            return None;
        }
        let end = |ws: &mut Workspace, cx: &mut Context<Workspace>| {
            ws.sidebar_drag = false;
            let mut s = ws.store.settings();
            s.sidebar_width = Some(ws.sidebar_width);
            let _ = ws.store.save_settings(&s);
            cx.notify();
        };
        Some(
            deferred(
                div()
                    .id("sidebar-resize-overlay")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .cursor(CursorStyle::ResizeLeftRight)
                    .on_mouse_move(cx.listener(move |ws, ev: &MouseMoveEvent, window, cx| {
                        // The release can land before this layer exists.
                        if ev.pressed_button != Some(MouseButton::Left) {
                            return end(ws, cx);
                        }
                        ws.sidebar_width = f32::from(ev.position.x);
                        ws.sidebar_width = ws.sidebar_w(window);
                        cx.notify();
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(move |ws, _, _, cx| end(ws, cx)))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(move |ws, _, _, cx| end(ws, cx))),
            )
            .with_priority(4)
            .into_any_element(),
        )
    }

    /// The note nav's own width handle, at the boundary between it and the
    /// folder column — same shape as `render_resize_handle`, a second strip.
    pub(crate) fn render_note_resize_handle(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let left = self.sidebar_w(window) + self.note_nav_w(window);
        let dragging = self.note_nav_drag;
        div()
            .id("note-nav-resize")
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(left - 6.))
            .w(px(15.))
            .group("note-nav-resize")
            .cursor(CursorStyle::ResizeLeftRight)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(5.))
                    .w(px(2.))
                    .rounded(px(1.))
                    .when(dragging, |d| d.bg(Theme::primary()))
                    .group_hover("note-nav-resize", |s| s.bg(Theme::primary())),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |ws, _, window, cx| {
                    cx.stop_propagation();
                    ws.note_nav_width = ws.note_nav_w(window);
                    ws.note_nav_drag = true;
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    /// While dragging, a transparent full-window layer keeps the resize cursor
    /// and follows the mouse — mirrors `render_resize_overlay` for the note nav.
    pub(crate) fn render_note_resize_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.note_nav_drag {
            return None;
        }
        let end = |ws: &mut Workspace, cx: &mut Context<Workspace>| {
            ws.note_nav_drag = false;
            let mut s = ws.store.settings();
            s.note_nav_width = Some(ws.note_nav_width);
            let _ = ws.store.save_settings(&s);
            cx.notify();
        };
        Some(
            deferred(
                div()
                    .id("note-nav-resize-overlay")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .cursor(CursorStyle::ResizeLeftRight)
                    .on_mouse_move(cx.listener(move |ws, ev: &MouseMoveEvent, window, cx| {
                        if ev.pressed_button != Some(MouseButton::Left) {
                            return end(ws, cx);
                        }
                        ws.note_nav_width = f32::from(ev.position.x) - ws.sidebar_w(window);
                        ws.note_nav_width = ws.note_nav_w(window);
                        cx.notify();
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(move |ws, _, _, cx| end(ws, cx)))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(move |ws, _, _, cx| end(ws, cx))),
            )
            .with_priority(4)
            .into_any_element(),
        )
    }

    /// Check or uncheck a 我的文档 row (entering 批量操作 if needed); false
    /// when the 50-page limit blocks it.
    pub(crate) fn toggle_check(&mut self, id: &str) -> bool {
        self.folded[1] = false;
        let sel = self.batch.get_or_insert_with(Vec::new);
        if let Some(i) = sel.iter().position(|x| x == id) {
            sel.remove(i);
        } else if sel.len() < BATCH_LIMIT {
            sel.push(id.to_string());
        } else {
            return false;
        }
        true
    }

    /// The checked docs, in list order.
    fn checked(&self) -> Vec<String> {
        let Some(sel) = &self.batch else { return vec![] };
        self.entries.iter().filter(|e| sel.contains(&e.id)).map(|e| e.id.clone()).collect()
    }

    /// 批量创建副本: a "… 副本" of each checked doc; leaves 批量操作.
    pub(crate) fn batch_copy(&mut self, cx: &mut Context<Self>) {
        let file_before = self.store.track_file_changes();
        let ids = self.checked();
        if ids.is_empty() {
            return;
        }
        for id in &ids {
            self.copy_doc(id, cx);
        }
        self.entries = self.store.list();
        self.sort();
        self.batch = None;
        self.row_menu = None;
        self.remember_files(file_before, crate::settings::tr("批量创建副本", "Duplicate selection"));
        cx.notify();
    }

    pub(crate) fn batch_move(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ids = self.checked();
        if !ids.is_empty() {
            self.open_move(ids, window, cx);
        }
    }

    pub(crate) fn ask_batch_delete(&mut self, cx: &mut Context<Self>) {
        let ids = self.checked();
        if !ids.is_empty() {
            self.confirm = Some(Confirm::Delete { ids, batch: true });
            self.row_menu = None;
            cx.notify();
        }
    }


    pub(crate) fn render_row_menu(&self, window: &Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(m) = self.row_menu.clone() else { return vec![] };
        if !m.batch && self.entries.iter().any(|e| e.id == m.id && e.meta.folder.is_some()) { return self.folder_row_menu(m, cx); }
        if !m.batch && self.entries.iter().any(|e| e.id == m.id && e.meta.local()) { return self.local_row_menu(m, cx); }
        let Some((pinned, starred)) = self.entries.iter().find(|e| e.id == m.id).map(|e| (e.meta.pinned, e.meta.starred > 0)) else { return vec![] };
        // The context menu: rows on the quiet hover fill.
        let item_c = |id: &'static str, ic: &'static str, label: &'static str, color: Hsla, icon_color: Hsla| {
            let g = SharedString::from(id);
            crate::ui::menu_row(id, g.clone())
                .mx(px(0.))
                .text_color(color)
                .child(icon(ic, 16., icon_color))
                .child(div().flex_1().min_w_0().child(crate::ui::menu_label(label, g, gpui::white())))
        };
        let item = |id, ic, label| item_c(id, ic, label, Theme::text(), Theme::icon());
        let divider = || div().mx(px(8.)).my(px(5.)).h(px(1.)).bg(Theme::line_border());
        if m.batch {
            // Right-click in 批量操作: the batch menu minus 添加快捷方式到, 转移所有权.
            let n = self.batch.as_ref().map_or(0, |b| b.len());
            let menu = popover()
                .id("batch-menu")
                .occlude()
                .w(px(ROW_MENU_W))
                .px(px(5.))
                .py(px(crate::ui::MENU_PAD_Y))
                .on_mouse_down_out(cx.listener(|w, _, _, cx| {
                    w.row_menu = None;
                    cx.notify();
                }))
                .child(div().h(px(ROW_ITEM)).px(px(8.)).flex().items_center().text_size(px(theme::ui_font_small())).text_color(Theme::text_placeholder()).child(crate::localized_format!("已选 {n} 项", "Selected: {n}")))
                .child(item("bm-copy", "copy", crate::settings::text("创建副本")).on_click(cx.listener(|w, _, _, cx| w.batch_copy(cx))))
                .child(item("bm-move", "folder-input", crate::settings::text("移动到")).on_click(cx.listener(|w, _, window, cx| w.batch_move(window, cx))))
                .child(item("bm-delete", "bin", crate::settings::text("删除")).on_click(cx.listener(|w, _, _, cx| w.ask_batch_delete(cx))));
            return vec![
                deferred(anchored().anchor(m.corner).position(m.pos).snap_to_window_with_margin(px(8.)).child(menu))
                    .with_priority(2)
                    .into_any_element(),
            ];
        }
        // Hovering 下载 opens its submenu; hovering any other item closes it.
        let set_sub = |open: bool| {
            cx.listener(move |w: &mut Workspace, hovered: &bool, _, cx| {
                if let Some(m) = w.row_menu.as_mut().filter(|m| *hovered && m.sub != open) {
                    m.sub = open;
                    cx.notify();
                }
            })
        };
        let (dup, mv, path, pin, star, dl, ren, del) = (m.id.clone(), m.id.clone(), m.id.clone(), m.id.clone(), m.id.clone(), m.id.clone(), m.id.clone(), m.id.clone());
        let pdf = m.id.clone();
        let new_window = m.id.clone();
        let sub_bounds = self.row_sub_bounds.clone();
        // The row menu leaves out the cloud entries 在新标签页打开, 分享,
        // 复制链接, 添加快捷方式到, 转移所有权, 设置密级.
        let menu = popover()
            .id("row-menu")
            .occlude()
            .w(px(ROW_MENU_W))
            .px(px(5.))
            .py(px(crate::ui::MENU_PAD_Y))
            .on_mouse_down_out(cx.listener(move |w, ev: &MouseDownEvent, _, cx| {
                if sub_bounds.get().is_none_or(|b| !b.contains(&ev.position)) {
                    w.row_menu = None;
                    cx.notify();
                }
            }))
            .child(
                item("rm-window", "copy", crate::settings::text("在新窗口中打开"))
                    .on_hover(set_sub(false))
                    .on_click(cx.listener(move |w, _, _, cx| {
                        w.row_menu = None;
                        let id = new_window.clone();
                        cx.defer(move |cx| crate::quick_note::open_document(Some(id), cx));
                        cx.notify();
                    })),
            )
            .child(
                item("rm-copy", "copy", crate::settings::text("创建副本"))
                    .on_hover(set_sub(false))
                    .on_click(cx.listener(move |w, _, window, cx| w.duplicate_doc(&dup, window, cx))),
            )
            .child(
                item("rm-move", "folder-input", crate::settings::text("移动到"))
                    .on_hover(set_sub(false))
                    .on_click(cx.listener(move |w, _, window, cx| w.open_move(vec![mv.clone()], window, cx))),
            )
            .child(
                item("rm-path", "copy", crate::settings::text("复制文件地址"))
                    .on_hover(set_sub(false))
                    .on_click(cx.listener(move |w, _, _, cx| {
                        w.row_menu = None;
                        if let Some(path) = w.store.file_path(&path) {
                            cx.write_to_clipboard(ClipboardItem::new_string(path.display().to_string()));
                            w.show_toast(crate::settings::text("已复制文件地址"), cx);
                        }
                    })),
            )
            .child(
                item("rm-pin", if pinned { "pin-off" } else { "pin" }, if pinned { crate::settings::text("从“置顶”移除") } else { crate::settings::text("添加到“置顶”") })
                    .on_hover(set_sub(false))
                    .on_click(cx.listener(move |w, _, _, cx| w.toggle_pin(&pin, cx))),
            )
            .child(
                item("rm-star", if starred { "star-off" } else { "star" }, if starred { crate::settings::text("取消收藏") } else { crate::settings::tr("收藏", "Add to favorites") })
                    .on_hover(set_sub(false))
                    .on_click(cx.listener(move |w, _, _, cx| w.toggle_star(&star, cx))),
            )
            .child(divider())
            .child(
                item("rm-download", "export", crate::settings::text("下载"))
                    .when(m.sub, |d| d.bg(Theme::fill_hover()))
                    .child(icon("chevron-right", 10., Theme::icon()))
                    .on_hover(set_sub(true)),
            )
            .child(
                item("rm-rename", "square-pen", crate::settings::text("重命名"))
                    .on_hover(set_sub(false))
                    .on_click(cx.listener(move |w, _, window, cx| w.start_rename(&ren, window, cx))),
            )
            .child(divider())
            .child(
                item_c("rm-del", "bin", crate::settings::text("删除"), Theme::danger(), Theme::danger())
                    .on_hover(set_sub(false))
                    .on_click(cx.listener(move |w, _, _, cx| w.ask_delete(&del, cx))),
            );
        let vh = f32::from(window.viewport_size().height);
        let menu_h = row_menu_height(ROW_ITEMS, ROW_DIVIDERS);
        // From the "…" button: hang 2px below it, or sit 2px above it when
        // there's no room below. From a right-click: at the pointer.
        let (origin, corner) = if m.pos.x < px(0.) {
            let (left, top) = (-m.pos.x, f32::from(m.pos.y));
            if top + 20. + 2. + menu_h > vh - 8. {
                (point(left, px(top - 2.)), Corner::BottomLeft)
            } else {
                (point(left, px(top + 22.)), Corner::TopLeft)
            }
        } else {
            (m.pos, m.corner)
        };
        let menu_top = match corner {
            Corner::BottomLeft | Corner::BottomRight => f32::from(origin.y) - menu_h,
            _ => f32::from(origin.y),
        };
        let mut out = vec![
            deferred(anchored().anchor(corner).position(origin).snap_to_window_with_margin(px(8.)).child(menu))
                .with_priority(2)
                .into_any_element(),
        ];
        if m.sub {
            let record = self.row_sub_bounds.clone();
            record.set(None);
            // Level the submenu's first item with 下载.
            let top = row_sub_top(menu_top);
            let left = match corner {
                Corner::TopRight | Corner::BottomRight => origin.x - px(ROW_MENU_W),
                _ => origin.x,
            };
            let x = left + px(ROW_MENU_W + 4.);
            out.push(
                deferred(
                    anchored().anchor(Corner::TopLeft).position(point(x, px(top))).snap_to_window_with_margin(px(8.)).child(
                        popover()
                            .id("rm-download-menu")
                            .occlude()
                            .w(px(130.))
                            .px(px(5.))
                            .py(px(crate::ui::MENU_PAD_Y))
                            .child(canvas(move |b, _, _| record.set(Some(b)), |_, _, _, _| {}).absolute().inset_0())
                            .when(crate::printing::SUPPORTED, |d| d.child(crate::docmenu::sub_item("rm-pdf", "PDF").on_click(cx.listener(move |w, _, window, cx| w.export_pdf(&pdf, window, cx)))))
                            .child(crate::docmenu::sub_item("rm-md", "Markdown").on_click(cx.listener(move |w, _, _, cx| w.open_export_md(&dl, cx)))),
                    ),
                )
                .with_priority(2)
                .into_any_element(),
            );
        } else {
            self.row_sub_bounds.set(None);
        }
        out
    }
}
