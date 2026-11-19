use crate::app::Workspace;
use crate::store::Entry;
use crate::theme::{self, Theme};
use crate::ui::{icon, ui_text};
use gpui::{prelude::FluentBuilder, *};
use std::{cell::Cell, collections::{HashMap, HashSet, VecDeque}, rc::Rc, time::Instant};

const PITCH: f32 = 31.;
const INDENT: f32 = 18.;

#[derive(Clone)]
struct Row { entry: Entry, depth: usize }

/// Seconds a dragged row must rest on a collapsed folder before it opens.
const DWELL: f32 = 0.45;

/// A damped spring per axis; the layout uses (x, y, width), the drop wash (x, y, width, height).
#[derive(Clone, Copy)]
pub(crate) struct Spring<const N: usize = 3> { pub p: [f32; N], pub v: [f32; N], pub target: [f32; N] }
impl<const N: usize> Spring<N> {
    pub(crate) fn new(p: [f32; N]) -> Self { Self { p, v: [0.; N], target: p } }
    pub(crate) fn step(&mut self, dt: f32) -> bool {
        let mut moving = false;
        for i in 0..N {
            // Substeps keep the same damped spring stable at 60 and 120 Hz.
            for _ in 0..4 {
                self.v[i] += (420. * (self.target[i] - self.p[i]) - 31. * self.v[i]) * dt / 4.;
                self.p[i] += self.v[i] * dt / 4.;
            }
            if (self.p[i] - self.target[i]).abs() < 0.05 && self.v[i].abs() < 0.1 { self.p[i] = self.target[i]; self.v[i] = 0.; }
            else { moving = true; }
        }
        moving
    }
}

/// A scalar spring for opacities and scales: `k` stiffness, `c` damping
/// (critical at 2√k; below that it overshoots, which is what the pops want).
#[derive(Clone, Copy)]
pub(crate) struct Fade { pub p: f32, pub v: f32, pub to: f32, k: f32, c: f32 }
impl Fade {
    pub(crate) const fn new(p: f32, k: f32, c: f32) -> Self { Self { p, v: 0., to: p, k, c } }
    pub(crate) fn from(mut self, p: f32, to: f32) -> Self { self.p = p; self.to = to; self.v = 0.; self }
    pub(crate) fn step(&mut self, dt: f32) -> bool {
        if self.p == self.to && self.v == 0. { return false; }
        for _ in 0..4 {
            self.v += (self.k * (self.to - self.p) - self.c * self.v) * dt / 4.;
            self.p += self.v * dt / 4.;
        }
        if (self.p - self.to).abs() < 0.002 && self.v.abs() < 0.02 { self.p = self.to; self.v = 0.; false } else { true }
    }
    pub(crate) fn unit(&self) -> f32 { self.p.clamp(0., 1.) }
}

const SHOW: Fade = Fade::new(1., 260., 30.);

/// Everything a row animates besides its place: entering, the glow after
/// landing, the icon's bounce and the card it sinks back from.
#[derive(Clone, Copy)]
struct RowFx { show: Fade, glow: Fade, pop: Fade, lift: Fade }
impl RowFx {
    fn new() -> Self {
        Self { show: SHOW, glow: Fade::new(0., 22., 9.4), pop: Fade::new(1., 600., 15.), lift: Fade::new(0., 170., 26.) }
    }
}

pub(crate) struct Drag {
    id: String,
    start: Point<Pixels>,
    at: Point<Pixels>,
    offset: Point<Pixels>,
    started: bool,
    excluded: HashSet<String>,
    index: usize,
    depth: usize,
    initial_depth: usize,
    parent: Option<String>,
    original_index: usize,
    original_depth: usize,
    hover: Option<(String, Instant)>,
    ghost: Spring,
    lift: Fade,
}

#[derive(Clone)]
struct PendingMove { id: String, parent: Option<String>, before: Option<String>, history: u64 }

#[derive(Default)]
pub(crate) struct SidebarDrag {
    pub active: Option<Drag>,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    springs: HashMap<String, Spring>,
    pending: VecDeque<PendingMove>,
    saving: bool,
    fx: HashMap<String, RowFx>,
    /// The press that is about to click a row landed on a folder's icon
    /// (its chevron), and whether ⌥ was down: that folds the whole subtree.
    pub icon_press: bool,
    pub icon_alt: bool,
    /// Folders with something in them (they get a chevron), rebuilt each frame.
    parents: HashSet<String>,
}

impl SidebarDrag {
    pub(crate) fn has_children(&self, id: &str) -> bool { self.parents.contains(id) }
    pub(crate) fn kick(&mut self, id: &str, v: f32) { if let Some(f) = self.fx.get_mut(id) { f.pop.v += v; } }
}

/// A row's face without its controls: the drag ghost and rows on their way out.
fn row_face(e: &Entry, open: bool) -> Div {
    div().h(px(crate::sidebar::ROW_H)).w_full().pl(px(6.)).flex().items_center()
        .child(div().size(px(20.)).flex_none().flex().items_center().justify_center().map(|d| match &e.icon {
            _ if e.meta.folder.is_some() => d.child(icon(if open { "folder-open" } else { "folder" }, crate::sidebar::ROW_ICON, Theme::icon())),
            _ if e.meta.markdown => d.child(icon("doc-md", crate::sidebar::ROW_ICON, Theme::icon())),
            Some(em) => d.text_size(px(13.)).line_height(px(20.)).child(em.clone()),
            None => d.child(icon("doc", crate::sidebar::ROW_ICON, Theme::icon())),
        }))
        .child(div().pl(px(9.)).flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_size(px(13.))
            .child(ui_text(crate::settings::display_title(&e.title))))
}

fn faded(shadows: Vec<BoxShadow>, a: f32) -> Vec<BoxShadow> {
    shadows.into_iter().map(|mut s| { s.color = s.color.opacity(a); s }).collect()
}

impl Workspace {
    fn drag_rows(&self) -> Vec<Row> {
        crate::tree::Tree::new(&self.entries).rows(|id| self.expanded.contains(id)).into_iter()
            .filter(|(e, _)| self.folder_drag.active.as_ref().is_none_or(|d| !d.started || !d.excluded.contains(&e.id)))
            .map(|(e, depth)| Row { entry: e.clone(), depth }).collect()
    }

    pub(crate) fn move_folder_drag(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = self.folder_drag.active.as_mut() else { return };
        d.at = at;
        if !d.started && (f32::from(at.x - d.start.x).hypot(f32::from(at.y - d.start.y)) >= 4.) {
            d.started = true; d.lift.to = 1.;
            self.folder_drag.icon_press = false;
            self.folder_drag.icon_alt = false;
            self.row_menu = None; self.create_menu = None;
            window.focus(&self.focus);
        }
        self.project_folder_drop();
        cx.notify();
    }

    fn project_folder_drop(&mut self) {
        let Some(bounds) = self.folder_drag.bounds.get() else { return };
        let rows = self.drag_rows();
        let Some(d) = self.folder_drag.active.as_mut().filter(|d| d.started) else { return };
        let y = f32::from(d.at.y - d.offset.y - bounds.top()) + 15.;
        let desired = (d.initial_depth as i32 + (f32::from(d.at.x - d.start.x) / INDENT).round() as i32).max(0) as usize;
        // Hit the visible folder row with the pointer, including collapsed folders.
        // Accept immediately; the dwell timer only controls automatic expansion.
        let pointer_y = f32::from(d.at.y - bounds.top());
        let folder = rows.iter().enumerate().find(|(i, r)| {
            let top = self.folder_drag.springs.get(&r.entry.id).map_or(
                (*i + usize::from(*i >= d.index)) as f32 * PITCH, |s| s.p[1]);
            r.entry.meta.folder.is_some() && pointer_y > top + 5. && pointer_y < top + 26.
                && f32::from(d.at.x - bounds.left()) >= r.depth as f32 * INDENT
        });
        if let Some((i, r)) = folder {
            if d.hover.as_ref().is_none_or(|(id, _)| *id != r.entry.id) {
                d.hover = Some((r.entry.id.clone(), Instant::now()));
                d.initial_depth = r.depth + 1; d.start.x = d.at.x;
                // The folder notices the row: a small bounce as its lid opens.
                if let Some(f) = self.folder_drag.fx.get_mut(&r.entry.id) { f.pop.v += 3.2; }
            }
            d.index = i + 1;
            d.depth = r.depth + 1;
            d.parent = Some(r.entry.id.clone());
            if d.hover.as_ref().is_some_and(|(_, t)| t.elapsed().as_secs_f32() >= DWELL) && self.expanded.insert(r.entry.id.clone())
                && let Some(f) = self.folder_drag.fx.get_mut(&r.entry.id) { f.pop.v += 4.; }
            return;
        }
        let slot_y = d.index as f32 * PITCH;
        let in_slot = y >= slot_y && y <= slot_y + PITCH;
        d.hover = None;
        let index = if in_slot { d.index } else {
            rows.iter().enumerate().filter(|(i, _)| y > (*i + usize::from(*i >= d.index)) as f32 * PITCH + 15.).count()
        };
        let prev = index.checked_sub(1).and_then(|i| rows.get(i));
        let max = prev.map_or(0, |r| r.depth + usize::from(r.entry.meta.folder.is_some()));
        let min = rows.get(index).map_or(0, |r| r.depth);
        let depth = desired.clamp(min.min(max), max);
        let parent = if depth == 0 { None } else { rows[..index].iter().rev().find(|r| r.depth + 1 == depth && r.entry.meta.folder.is_some()).map(|r| r.entry.id.clone()) };
        d.index = index; d.depth = depth; d.parent = parent;
    }

    /// Where something dragged in from outside the tree lands at `at`: into the
    /// folder row under it, or a slot between rows (as a row being moved gets),
    /// beside the row below it. `local`: a 本机文件 row, which lands only on the
    /// tree, not back in its own section.
    pub(crate) fn sidebar_target(&self, at: Point<Pixels>, local: bool) -> Option<crate::sidebar_drop::SidebarDropAt> {
        use crate::sidebar_drop::SidebarDropAt;
        if !self.list_scroll.bounds().contains(&at) { return None; }
        let Some(b) = self.folder_drag.bounds.get().filter(|_| !self.folded[1]) else { return (!local).then(SidebarDropAt::top) };
        let rows = self.drag_rows();
        let y = f32::from(at.y - b.top());
        // Below the tree: the end of 我的文档 (a 本机文件 row over its own section: nowhere).
        if y > (rows.len() + 1) as f32 * PITCH + 6. { return (!local).then_some(SidebarDropAt { parent: None, slot: Some((rows.len(), 0)), before: None }); }
        // Rows where they stand with the open slot, as for a row being moved: the hit stays put while they make room.
        let cur = self.sidebar_drop.as_ref().and_then(|t| t.slot).map(|s| s.0);
        let top_of = |i: usize| (i + usize::from(cur.is_some_and(|c| i >= c))) as f32 * PITCH;
        if let Some((_, r)) = rows.iter().enumerate().find(|(i, r)| r.entry.meta.folder.is_some() && y > top_of(*i) + 5. && y < top_of(*i) + 26.) {
            return Some(SidebarDropAt { parent: Some(r.entry.id.clone()), slot: None, before: None });
        }
        let index = rows.iter().enumerate().filter(|(i, _)| y > top_of(*i) + 15.).count();
        Some(match rows.get(index) {
            Some(r) => SidebarDropAt { parent: r.entry.meta.parent.clone(), slot: Some((index, r.depth)), before: Some(r.entry.id.clone()) },
            None => SidebarDropAt { parent: None, slot: Some((index, 0)), before: None },
        })
    }

    pub(crate) fn end_folder_drag(&mut self, cancel: bool, cx: &mut Context<Self>) {
        let rows = self.drag_rows();
        let Some(d) = self.folder_drag.active.as_mut() else { return };
        if !d.started { self.folder_drag.active = None; cx.notify(); return; }
        let b = self.list_scroll.bounds();
        let cancel = cancel || d.at.x < b.left() || d.at.x > b.right() || d.at.y < b.top() || d.at.y > b.bottom();
        let before = rows.get(d.index).filter(|r| r.entry.meta.parent == d.parent).map(|r| r.entry.id.clone());
        let mut movement = PendingMove { id: d.id.clone(), parent: d.parent.clone(), before, history: 0 };
        let depth = if cancel { d.original_depth } else { d.depth };
        let index = if cancel { d.original_index } else { d.index };
        // Drop the overlay immediately: normal row color and hit testing resume this frame.
        // Seed the real row at the pointer so it settles without a second visual swap.
        let bounds = self.folder_drag.bounds.get();
        // The card keeps its lifted size and shadow and sinks back from there.
        let lift = d.lift.unit();
        let mut spring = d.ghost;
        if let Some(b) = bounds { spring.p[0] -= f32::from(b.left()); spring.p[1] -= f32::from(b.top()); }
        spring.p[0] -= 3. * lift; spring.p[1] -= lift; spring.p[2] += 6. * lift;
        spring.target = [depth as f32 * INDENT, index as f32 * PITCH, spring.p[2]];
        self.folder_drag.springs.insert(movement.id.clone(), spring);
        let fx = self.folder_drag.fx.entry(movement.id.clone()).or_insert_with(RowFx::new);
        fx.show = SHOW;
        fx.lift = fx.lift.from(lift, 0.);
        if !cancel { fx.glow = fx.glow.from(1., 0.); }
        if !cancel && let Some(parent) = &movement.parent { self.folder_drag.kick(parent, 4.5); }
        self.folder_drag.active = None;
        if !cancel {
            if let Some(parent) = &movement.parent { self.expanded.insert(parent.clone()); }
            let file_before = self.entries.clone();
            self.apply_tree_move(&movement);
            movement.history = self.remember_tree_move(file_before, crate::settings::tr("拖拽移动", "Drag move"));
            self.folder_drag.pending.push_back(movement);
            self.save_next_tree_move(cx);
            self.sync_sub_pages(cx);
        }
        cx.notify();
    }

    /// Put `id` last in `folder` (a drop on 主页's folder card), saved like a sidebar drop.
    pub(crate) fn move_into_folder(&mut self, id: &str, folder: &str, cx: &mut Context<Self>) {
        let mut movement = PendingMove { id: id.to_string(), parent: Some(folder.to_string()), before: None, history: 0 };
        let file_before = self.entries.clone();
        self.apply_tree_move(&movement);
        movement.history = self.remember_tree_move(file_before, crate::settings::tr("拖拽移动", "Drag move"));
        self.folder_drag.pending.push_back(movement);
        self.save_next_tree_move(cx);
        self.sync_sub_pages(cx);
        cx.notify();
    }


    pub(crate) fn reapply_pending_tree_moves(&mut self) {
        for movement in self.folder_drag.pending.clone() { self.apply_tree_move(&movement); }
    }

    fn apply_tree_move(&mut self, movement: &PendingMove) {
        let tree = crate::tree::Tree::new(&self.entries);
        let mut siblings: Vec<String> = tree.children(movement.parent.as_deref()).iter()
            .filter(|e| e.id != movement.id).map(|e| e.id.clone()).collect();
        let pos = movement.before.as_ref().and_then(|id| siblings.iter().position(|key| key == id)).unwrap_or(siblings.len());
        siblings.insert(pos, movement.id.clone());
        for e in &mut self.entries {
            if e.id == movement.id { e.meta.parent = movement.parent.clone(); }
            if let Some(i) = siblings.iter().position(|id| *id == e.id) {
                if movement.parent.is_none() { e.meta.root_order = Some(i as u64 + 1); }
                else { e.meta.order = i as u64 + 1; e.meta.root_order = None; }
            }
        }
    }

    pub(crate) fn tree_writes_pending(&self) -> bool { self.folder_drag.saving || !self.folder_drag.pending.is_empty() }

    fn save_next_tree_move(&mut self, cx: &mut Context<Self>) {
        if self.folder_drag.saving { return; }
        let Some(movement) = self.folder_drag.pending.front().cloned() else { return };
        self.folder_drag.saving = true;
        let history = movement.history;
        let mut writer = self.store.tree_writer();
        let task = cx.background_executor().spawn(async move {
            let journal = writer.track_file_changes();
            let result = writer.move_in_tree(&movement.id, movement.parent.as_deref(), movement.before.as_deref());
            let restored = if result.is_err() { Some(writer.list()) } else { None };
            (result, restored, journal.take())
        });
        cx.spawn(async move |this, cx| {
            let (result, restored, changes) = task.await;
            let _ = this.update(cx, |w, cx| {
                w.folder_drag.pending.pop_front();
                w.folder_drag.saving = false;
                if let Err(ref e) = result {
                    w.forget_file_change(history);
                    // Restore only hierarchy, preserving edits made while the write was pending.
                    if let Some(restored) = restored {
                        for entry in &mut w.entries {
                            if let Some(saved) = restored.iter().find(|e| e.id == entry.id) {
                                entry.meta.parent = saved.meta.parent.clone();
                                entry.meta.order = saved.meta.order;
                                entry.meta.root_order = saved.meta.root_order;
                            }
                        }
                    }
                    for movement in w.folder_drag.pending.clone() { w.apply_tree_move(&movement); }
                    w.sync_sub_pages(cx);
                    w.show_toast_kind(crate::ui::ToastKind::Error, &e.to_string(), cx);
                }
                if result.is_ok() { w.confirm_tree_history(history, changes); }
                w.save_next_tree_move(cx);
                cx.notify();
            });
        }).detach();
    }


    /// A click on a row. A folder opens its page and unfolds, like a page does;
    /// its chevron (or clicking the folder already shown) only folds and unfolds
    /// it, and ⌥ on the chevron does the same to every folder inside.
    fn sidebar_click(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let on_icon = std::mem::take(&mut self.folder_drag.icon_press) && self.folder_drag.has_children(id);
        let all = std::mem::take(&mut self.folder_drag.icon_alt);
        if self.entries.iter().any(|e| e.id == id && e.meta.folder.is_some()) {
            let shown = self.view == crate::app::View::Folder && self.folder_current.as_deref() == Some(id);
            if on_icon || shown {
                let opening = !self.expanded.contains(id);
                let folders: Vec<String> = if on_icon && all {
                    crate::tree::Tree::new(&self.entries).subtree(id).into_iter()
                        .filter(|k| self.entries.iter().any(|e| &e.id == k && e.meta.folder.is_some())).collect()
                } else { vec![id.to_string()] };
                for f in folders { if opening { self.expanded.insert(f); } else { self.expanded.remove(&f); } }
            } else if !self.open_folder(id, window, cx) { return; }
            self.folder_drag.kick(id, 3.5);
            cx.notify();
        } else { self.open(id, window, cx); }
    }

    pub(crate) fn track_sidebar_press(&mut self, ev: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.folder_drag.active.as_ref().is_some_and(|d| !d.started) { return; }
        if ev.pressed_button == Some(MouseButton::Left) { self.move_folder_drag(ev.position, window, cx); }
        else { self.folder_drag.active = None; }
    }

    pub(crate) fn finish_sidebar_click(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let click = self.folder_drag.active.as_ref().filter(|d| !d.started).map(|d| d.id.clone());
        if let Some(id) = click { self.folder_drag.active = None; self.sidebar_click(&id, window, cx); }
    }

    pub(crate) fn render_folder_drag_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.folder_drag.active.as_ref().filter(|d| d.started)?;
        let mut layer = div().id("folder-drag-overlay").absolute().inset_0().occlude()
            .on_mouse_move(cx.listener(|w, ev: &MouseMoveEvent, window, cx| {
                if ev.pressed_button != Some(MouseButton::Left) { w.end_folder_drag(true, cx); return; }
                w.move_folder_drag(ev.position, window, cx);
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|w, _, window, cx| {
                let click = w.folder_drag.active.as_ref().filter(|d| !d.started).map(|d| d.id.clone());
                if let Some(id) = click { w.folder_drag.active = None; w.sidebar_click(&id, window, cx); }
                else { w.end_folder_drag(false, cx); }
            }));
        if d.started && let Some(e) = self.entries.iter().find(|e| e.id == d.id) {
            // Lifted: a touch wider and higher than the row, with a shadow that grows in.
            let [x, y, width] = d.ghost.p;
            let l = d.lift.p.max(0.);
            let inside = d.excluded.len().saturating_sub(1);
            layer = layer.child(div().absolute().left(px(x - 3. * l)).top(px(y - l)).w(px(width + 6. * l)).h(px(crate::sidebar::ROW_H)).pr(px(6.)).rounded(px(crate::sidebar::ROW_R))
                .bg(theme::raised(Theme::bg_body())).shadow(faded(theme::shadow_panel(), l.min(1.))).flex().items_center().text_color(Theme::text())
                .child(row_face(e, self.expanded.contains(&e.id)).flex_1().min_w_0().pl(px(6. + 3. * l)))
                .when(inside > 0, |g| g.child(div().flex_none().h(px(18.)).min_w(px(18.)).px(px(5.)).rounded(px(9.)).flex().items_center().justify_center()
                    .bg(Theme::sidebar_drop_slot()).text_size(px(11.)).text_color(Theme::sidebar_text()).font_weight(FontWeight::MEDIUM)
                    .opacity(d.lift.unit()).child(inside.to_string()))));
        }
        Some(deferred(layer).with_priority(5).into_any_element())
    }
}
