//! Capture History window: browse, filter, restore, annotate, copy, pin
//! or delete recent captures (kept for the configured retention period).

use crate::app::{self, state, state_mut, Capture};
use crate::history::{CaptureKind, HistoryEntry};
use crate::ui::{self, color, icon, Icon};
use crate::{annotate, pin, platform, quick};
use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    Kind(CaptureKind),
}

pub struct HistoryView {
    filter: Filter,
    hovered: Option<u64>,
    selected: Option<u64>,
    focus: FocusHandle,
}

pub fn open(cx: &mut App) {
    if let Some(h) = state(cx).history_window {
        if h.update(cx, |_, window, _| window.activate_window()).is_ok() {
            return;
        }
    }
    let bounds = Bounds::centered(None, size(px(1040.), px(700.)), cx);
    let res = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions { title: Some("Snipsy — Capture History".into()), appears_transparent: true, traffic_light_position: None }),
            window_min_size: Some(size(px(560.), px(420.))),
            app_id: Some("Snipsy".into()),
            ..Default::default()
        },
        |window, cx| {
            if let Some(h) = platform::hwnd_of(window) {
                platform::force_foreground(h);
            }
            cx.new(|cx| {
                let focus = cx.focus_handle();
                window.focus(&focus);
                HistoryView { filter: Filter::All, hovered: None, selected: None, focus }
            })
        },
    );
    if let Ok(h) = res {
        state_mut(cx).history_window = Some(h);
    }
}

pub fn refresh(cx: &mut App) {
    if let Some(h) = state(cx).history_window {
        let _ = h.update(cx, |_, _, cx| cx.notify());
    }
}

/// Load a history entry's full image in the background and hand it to `then`.
fn with_capture(e: &HistoryEntry, cx: &mut App, then: impl FnOnce(Capture, &mut App) + 'static) {
    let path = e.file.clone();
    let (kind, created) = (e.kind, e.created);
    let p2 = path.clone();
    let task = cx.background_executor().spawn(async move { image::open(&p2).map(|i| i.to_rgba8()) });
    cx.spawn(async move |cx: &mut AsyncApp| {
        if let Ok(img) = task.await {
            let _ = cx.update(|cx| {
                let mut c = Capture::new(Arc::new(img), kind);
                c.created = created;
                // Keep the fresh id from Capture::new: the entry may be restored
                // more than once, and Quick Access needs unique ids.
                c.file = Some(path);
                then(c, cx);
            });
        }
    })
    .detach();
}

impl HistoryView {
    fn entries(&self, cx: &App) -> Vec<HistoryEntry> {
        state(cx)
            .history
            .entries
            .iter()
            .filter(|e| match self.filter {
                Filter::All => true,
                Filter::Kind(k) => e.kind == k || (k == CaptureKind::Area && e.kind == CaptureKind::Timer),
            })
            .cloned()
            .collect()
    }

    fn restore(&mut self, e: &HistoryEntry, cx: &mut Context<Self>) {
        with_capture(e, cx, |c, cx| quick::push(c, cx));
    }
}

impl Render for HistoryView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entries = self.entries(cx);
        let total = state(cx).history.entries.len();
        let days = state(cx).settings.history_days;

        let pill = |id: &'static str, label: &'static str, active: bool| {
            div()
                .id(id)
                .px(px(14.))
                .h(px(30.))
                .flex()
                .items_center()
                .rounded_full()
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .when(active, |d| d.bg(color::accent()).text_color(rgb(0xFFFFFF)))
                .when(!active, |d| d.bg(rgba(0xFFFFFF14)).text_color(color::text_dim()).hover(|s| s.bg(rgba(0xFFFFFF24))))
                .child(label)
        };
        let filters = [
            ("f-all", "All", Filter::All),
            ("f-area", "Area", Filter::Kind(CaptureKind::Area)),
            ("f-full", "Fullscreen", Filter::Kind(CaptureKind::Fullscreen)),
            ("f-win", "Window", Filter::Kind(CaptureKind::Window)),
            ("f-edit", "Edited", Filter::Kind(CaptureKind::Edited)),
        ];
        let mut bar = div().flex().gap(px(8.)).items_center();
        for (id, label, f) in filters {
            bar = bar.child(pill(id, label, self.filter == f).on_click(cx.listener(move |this, _, _, cx| {
                this.filter = f;
                cx.notify();
            })));
        }

        let mut grid = div().flex().flex_wrap().gap(px(18.)).p(px(24.));
        for e in entries.iter() {
            let id = e.id;
            let hovered = self.hovered == Some(id);
            let selected = self.selected == Some(id);
            let action = |aid: (&'static str, u64), path: Icon| {
                div()
                    .id(aid)
                    .size(px(30.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgba(0x1E1E20D9))
                    .border_1()
                    .border_color(rgba(0xFFFFFF26))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(0x3A3A3CF2)))
                    .child(icon(path, px(14.), rgb(0xFFFFFF)))
            };
            let e1 = e.clone();
            let e2 = e.clone();
            let e3 = e.clone();
            let e4 = e.clone();
            let e5 = e.clone();
            grid = grid.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .items_center()
                    .child(
                        div()
                            .id(("h", id))
                            .w(px(226.))
                            .h(px(150.))
                            .relative()
                            .rounded(px(14.))
                            .overflow_hidden()
                            .border(px(if selected { 3. } else { 1. }))
                            .border_color(if selected { color::accent().into() } else { rgba(0xFFFFFF1F) })
                            .bg(color::group_bg())
                            .cursor_pointer()
                            .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                                if *h {
                                    this.hovered = Some(id);
                                } else if this.hovered == Some(id) {
                                    this.hovered = None;
                                }
                                cx.notify();
                            }))
                            .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                                this.selected = Some(id);
                                if ev.click_count() >= 2 {
                                    with_capture(&e1, cx, |c, cx| annotate::open(c, cx));
                                }
                                cx.notify();
                            }))
                            .child(img(e.thumb.clone()).absolute().size_full().object_fit(ObjectFit::Cover))
                            .when(hovered, |d| {
                                d.child(
                                    div()
                                        .absolute()
                                        .size_full()
                                        .bg(rgba(0x00000059))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .gap(px(8.))
                                        .child(action(("ha", id), Icon::Edit).on_click(cx.listener(move |_, _, _, cx| {
                                            cx.stop_propagation();
                                            with_capture(&e2, cx, |c, cx| annotate::open(c, cx));
                                        })))
                                        .child(action(("hc", id), Icon::Copy).on_click(cx.listener(move |_, _, _, cx| {
                                            cx.stop_propagation();
                                            with_capture(&e3, cx, |c, cx| app::copy_capture(&c, cx));
                                        })))
                                        .child(action(("hp", id), Icon::Pin).on_click(cx.listener(move |_, _, _, cx| {
                                            cx.stop_propagation();
                                            with_capture(&e4, cx, |c, cx| pin::open(c, cx));
                                        })))
                                        .child(action(("hd", id), Icon::Trash).on_click(cx.listener(move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            state_mut(cx).history.remove(id);
                                            this.hovered = None;
                                            cx.notify();
                                        }))),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .h(px(28.))
                            .when(selected || hovered, |d| {
                                d.child(
                                    div()
                                        .id(("restore", id))
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .px(px(12.))
                                        .h(px(28.))
                                        .rounded(px(8.))
                                        .bg(color::accent())
                                        .text_color(rgb(0xFFFFFF))
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .cursor_pointer()
                                        .hover(|s| s.bg(color::accent_hover()))
                                        .child(icon(Icon::Restore, px(14.), rgb(0xFFFFFF)))
                                        .child("Restore")
                                        .on_click(cx.listener(move |this, _, _, cx| this.restore(&e5, cx))),
                                )
                            })
                            .when(!(selected || hovered), |d| {
                                d.child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(color::text_faint())
                                        .child(format!("{} · {} · {}×{}", e.kind.label(), e.created.format("%b %d, %H:%M"), e.width, e.height)),
                                )
                            }),
                    ),
            );
        }

        let empty = entries.is_empty();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(color::window_bg())
            .font_family(ui::FONT)
            .text_color(color::text())
            .track_focus(&self.focus)
            .child(ui::title_bar("Capture History", None, None))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(24.))
                    .pt(px(18.))
                    .child(bar)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(div().text_size(px(12.)).text_color(color::text_faint()).child(format!("{total} captures · kept for {days} days")))
                            .child(ui::button("open-folder", "Open Folder", false).on_click(|_, _, _| {
                                platform::open_path(&crate::settings::data_dir().join("history"));
                            }))
                            .child(ui::button("clear", "Clear All", false).on_click(cx.listener(|_, _, _, cx| {
                                state_mut(cx).history.clear();
                                cx.notify();
                            }))),
                    ),
            )
            .child(
                div()
                    .id("scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .when(empty, |d| {
                        d.flex().items_center().justify_center().child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(10.))
                                .child(icon(Icon::History, px(40.), color::text_faint()))
                                .child(div().text_size(px(15.)).text_color(color::text_dim()).child("No captures yet"))
                                .child(div().text_size(px(12.)).text_color(color::text_faint()).child("Screenshots you take will appear here.")),
                        )
                    })
                    .when(!empty, |d| d.child(grid)),
            )
    }
}
