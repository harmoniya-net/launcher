use std::sync::Arc;

use gpui::{
    App, Context, Entity, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Render, Styled, Window, div, px, relative,
};

use harmoniya_launch::pipeline::{LaunchError, LaunchProgress, LaunchState};
use crate::state::AppState;
use crate::theme::Theme;
use crate::widgets::modal::Modal;

type Callback = Arc<dyn Fn(&mut App) + 'static>;

pub struct LaunchModal {
    state: Entity<AppState>,
    on_close: Callback,
}

impl LaunchModal {
    pub fn new(state: Entity<AppState>, on_close: impl Fn(&mut App) + 'static, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state, on_close: Arc::new(on_close) }
    }
}

fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.;
    const MB: f64 = KB * 1024.;
    const GB: f64 = MB * 1024.;
    let f = n as f64;
    if f < KB {
        format!("{n} B")
    } else if f < MB {
        format!("{:.1} KB", f / KB)
    } else if f < GB {
        format!("{:.1} MB", f / MB)
    } else {
        format!("{:.2} GB", f / GB)
    }
}

fn file_label(path: &str) -> String {
    match path.rsplit(['/', '\\']).next() {
        Some(name) => name.to_string(),
        None => path.to_string(),
    }
}

fn starting_view() -> gpui::AnyElement {
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .text_color(Theme::text_muted())
        .text_size(px(14.))
        .child("Підготовка…")
        .into_any_element()
}

fn progress_view(p: &LaunchProgress) -> gpui::AnyElement {
    let mut block = div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            div()
                .text_size(px(14.))
                .text_color(Theme::text())
                .child(p.phase.label()),
        )
        .child(
            // Track
            div()
                .w_full()
                .h(px(8.))
                .bg(Theme::surface_raised())
                .rounded(px(4.))
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .w(relative(p.percent as f32 / 100.))
                        .bg(Theme::text())
                        .rounded(px(4.)),
                ),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(Theme::text_muted())
                .text_right()
                .child(format!("{}%", p.percent)),
        );

    if !p.files.is_empty() {
        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .pt(px(12.))
            .border_t_1()
            .border_color(Theme::surface_raised());

        for f in &p.files {
            let bytes = match f.total {
                Some(total) => format!("{} / {}", format_bytes(f.bytes), format_bytes(total)),
                None => format_bytes(f.bytes),
            };
            let mut row = div().flex().flex_col().gap(px(4.)).text_size(px(12.)).child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_color(Theme::text())
                            .child(file_label(&f.path)),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(Theme::text_muted())
                            .child(bytes),
                    ),
            );
            if let Some(total) = f.total.filter(|t| *t > 0) {
                let frac = (f.bytes as f32 / total as f32).clamp(0., 1.);
                row = row.child(
                    div()
                        .w_full()
                        .h(px(3.))
                        .bg(Theme::surface_raised())
                        .rounded(px(2.))
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(relative(frac))
                                .bg(Theme::text_muted())
                                .rounded(px(2.)),
                        ),
                );
            }
            list = list.child(row);
        }
        block = block.child(list);
    }

    block.into_any_element()
}

fn error_view(e: &LaunchError, retry: impl Fn(&mut App) + 'static) -> gpui::AnyElement {
    let retry = Arc::new(retry);
    let code_line = {
        let mut s = format!("{:?}", e.code).to_uppercase();
        if let Some(phase) = e.phase {
            s = format!("{s} · {}", phase.short());
        }
        s
    };

    let mut block = div()
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .gap(px(12.))
                .child(
                    div()
                        .text_size(px(16.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::status_offline())
                        .child(e.code.label()),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(Theme::text_muted())
                        .child(code_line),
                ),
        )
        .child(
            div()
                .text_size(px(13.))
                .text_color(Theme::text())
                .child(e.message.clone()),
        );

    if !e.paths.is_empty() {
        let mut paths = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .p(px(10.))
            .rounded(Theme::radius_card())
            .border_1()
            .border_color(Theme::surface_raised())
            .text_size(px(11.))
            .text_color(Theme::text_muted())
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(Theme::text())
                    .child(format!("Файли ({})", e.paths.len())),
            );
        for p in e.paths.iter().take(50) {
            paths = paths.child(div().truncate().child(p.clone()));
        }
        block = block.child(paths);
    }

    block = block.child(
        div().flex().gap(px(8.)).pt(px(4.)).child(
            div()
                .id("launch-retry")
                .px(px(20.))
                .py(px(10.))
                .rounded(px(2.))
                .bg(Theme::text())
                .text_color(gpui::rgb(0x0e0d0f))
                .text_size(px(13.))
                .font_weight(FontWeight::BOLD)
                .cursor_pointer()
                .hover(|s| s.bg(gpui::rgb(0xffffff)))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    retry(cx);
                })
                .child("Повторити"),
        ),
    );

    block.into_any_element()
}

impl Render for LaunchModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let title = state.selected_modpack().map(|m| m.title.clone()).unwrap_or_else(|| "Запуск".into());
        let on_close = self.on_close.clone();

        let inner = match &state.launch_state {
            LaunchState::Progress(p) => progress_view(p),
            LaunchState::Error(e) => {
                let handle = self.state.clone();
                error_view(e, move |cx| {
                    handle.update(cx, |s, cx| {
                        s.launch_state = LaunchState::Idle;
                        s.start_launch(cx);
                    });
                })
            }
            // Starting, plus Idle/Done as a brief transition before the modal closes.
            _ => starting_view(),
        };

        let body = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .flex_1()
            .min_h(px(0.))
            .p(px(24.))
            .overflow_hidden()
            .child(inner);

        Modal::new(body)
            .title(title)
            .size(760., 560.)
            .on_close(move |cx| on_close(cx))
            .render()
    }
}
