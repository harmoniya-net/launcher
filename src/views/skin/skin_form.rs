use std::path::PathBuf;

use gpui::{
    Context, Entity, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use harmoniya_api::auth;
use harmoniya_api::services::yggdrasil::{self, SkinModel};
use crate::state::AppState;
use crate::theme::Theme;

const NO_FILE: &str = "файл не обрано";

#[derive(Default, Clone)]
struct EditorState {
    user_model: Option<SkinModel>,
    pending_skin: Option<(PathBuf, String)>,
    pending_cape: Option<(PathBuf, String)>,
    status: Option<(String, Option<bool>)>,
    saving: bool,
}

pub struct SkinForm {
    state: Entity<AppState>,
    editor: EditorState,
}

impl SkinForm {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state, editor: EditorState::default() }
    }

    fn current_model(&self, cx: &gpui::App) -> SkinModel {
        self.editor.user_model.unwrap_or_else(|| self.state.read(cx).current_skin_model())
    }
}

impl Render for SkinForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let profile_skin = self.state.read(cx).skin_profile.as_ref().and_then(|p| p.skin_url.clone());
        let profile_cape = self.state.read(cx).skin_profile.as_ref().and_then(|p| p.cape_url.clone());
        let model = self.current_model(cx);
        let saving = self.editor.saving;
        let status = self.editor.status.clone();

        let skin_name = self.editor.pending_skin.as_ref().map(|(_, n)| n.clone()).unwrap_or_else(|| NO_FILE.into());
        let cape_name = self.editor.pending_cape.as_ref().map(|(_, n)| n.clone()).unwrap_or_else(|| NO_FILE.into());

        let has_pending_skin = self.editor.pending_skin.is_some();
        let has_pending_cape = self.editor.pending_cape.is_some();
        let pending_model_change = self.editor.user_model.is_some()
            && self.editor.user_model != Some(self.state.read(cx).current_skin_model());
        let has_changes = has_pending_skin || has_pending_cape || pending_model_change;

        let pick_skin = cx.entity().clone();
        let pick_cape = cx.entity().clone();
        let on_save = cx.entity().clone();
        let reset_skin_handle = cx.entity().clone();
        let reset_cape_handle = cx.entity().clone();
        let model_classic = cx.entity().clone();
        let model_slim = cx.entity().clone();

        div()
            .id("skin-form-scroll")
            .flex_1()
            .p(px(40.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(32.))
            .child(
                div()
                    .text_size(px(28.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::text())
                    .child("Скін"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(20.))
                    .child(file_field("ФАЙЛ СКІНУ", skin_name, move |_, _, cx| {
                        if let Some(path) = pick_png() {
                            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                            let bytes = std::fs::read(&path).ok().map(std::sync::Arc::new);
                            pick_skin.update(cx, |this, cx| {
                                this.editor.pending_skin = Some((path, name));
                                if let Some(b) = bytes {
                                    this.state.update(cx, |s, cx| s.set_preview_skin_bytes(Some(b), cx));
                                }
                                cx.notify();
                            });
                        }
                    }))
                    .child(model_field(model, {
                        let h = model_classic;
                        move |_, _, cx| {
                            h.update(cx, |this, cx| {
                                this.editor.user_model = Some(SkinModel::Classic);
                                this.state.update(cx, |s, cx| s.set_preview_skin_model(SkinModel::Classic, cx));
                                cx.notify();
                            });
                        }
                    }, {
                        let h = model_slim;
                        move |_, _, cx| {
                            h.update(cx, |this, cx| {
                                this.editor.user_model = Some(SkinModel::Slim);
                                this.state.update(cx, |s, cx| s.set_preview_skin_model(SkinModel::Slim, cx));
                                cx.notify();
                            });
                        }
                    }))
                    .child(file_field("ПЛАЩ", cape_name, move |_, _, cx| {
                        if let Some(path) = pick_png() {
                            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                            let bytes = std::fs::read(&path).ok().map(std::sync::Arc::new);
                            pick_cape.update(cx, |this, cx| {
                                this.editor.pending_cape = Some((path, name));
                                if let Some(b) = bytes {
                                    this.state.update(cx, |s, cx| s.set_preview_cape_bytes(Some(b), cx));
                                }
                                cx.notify();
                            });
                        }
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .child(
                        action_button("Зберегти", !saving && has_changes, Theme::accent(), move |_, _, cx| {
                            on_save.update(cx, |this, cx| this.save(cx));
                        }),
                    )
                    .child(
                        if let Some((text, ok)) = status {
                            let color = match ok {
                                Some(true) => Theme::status_online(),
                                Some(false) => Theme::status_offline(),
                                None => Theme::text_muted(),
                            };
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(color)
                                .child(text)
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        },
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(18.))
                    .child(
                        reset_link("Скинути скін", !saving && profile_skin.is_some(), move |_, _, cx| {
                            reset_skin_handle.update(cx, |this, cx| this.reset(Kind::Skin, cx));
                        }),
                    )
                    .child(
                        reset_link("Скинути плащ", !saving && profile_cape.is_some(), move |_, _, cx| {
                            reset_cape_handle.update(cx, |this, cx| this.reset(Kind::Cape, cx));
                        }),
                    ),
            )
    }
}

enum Kind { Skin, Cape }

impl SkinForm {
    fn save(&mut self, cx: &mut Context<Self>) {
        if self.editor.saving { return; }
        let Some(tokens) = self.state.read(cx).tokens.clone() else { return; };
        let model = self.current_model(cx);
        let pending_skin = self.editor.pending_skin.clone();
        let pending_cape = self.editor.pending_cape.clone();

        self.editor.saving = true;
        self.editor.status = Some(("Збереження...".into(), None));
        cx.notify();

        cx.spawn(async move |this, cx| {
            let res: anyhow::Result<auth::tokens::Tokens> = harmoniya_api::http::on_tokio(async move {
                let access = ensure_token(tokens).await?;
                if let Some((path, name)) = pending_skin {
                    let bytes = std::fs::read(&path)?;
                    yggdrasil::upload_skin(&access.access_token, bytes, name, model).await?;
                }
                if let Some((path, name)) = pending_cape {
                    let bytes = std::fs::read(&path)?;
                    yggdrasil::upload_cape(&access.access_token, bytes, name).await?;
                }
                Ok(access)
            }).await;
            this.update(cx, |this, cx| {
                this.editor.saving = false;
                match res {
                    Ok(tokens) => {
                        this.editor.pending_skin = None;
                        this.editor.pending_cape = None;
                        this.editor.status = Some(("Збережено!".into(), Some(true)));
                        this.state.update(cx, |s, cx| {
                            s.tokens = Some(tokens);
                            let _ = harmoniya_api::auth::storage::save(s.tokens.as_ref().unwrap());
                            s.fetch_skin_profile(cx);
                        });
                    }
                    Err(e) => {
                        this.editor.status = Some((e.to_string(), Some(false)));
                    }
                }
                cx.notify();
            }).ok();
        }).detach();
    }

    fn reset(&mut self, kind: Kind, cx: &mut Context<Self>) {
        if self.editor.saving { return; }
        let Some(tokens) = self.state.read(cx).tokens.clone() else { return; };

        self.editor.saving = true;
        self.editor.status = Some(("Скидання...".into(), None));
        cx.notify();

        cx.spawn(async move |this, cx| {
            let res: anyhow::Result<auth::tokens::Tokens> = harmoniya_api::http::on_tokio(async move {
                let access = ensure_token(tokens).await?;
                match kind {
                    Kind::Skin => yggdrasil::reset_skin(&access.access_token).await?,
                    Kind::Cape => yggdrasil::reset_cape(&access.access_token).await?,
                }
                Ok(access)
            }).await;
            this.update(cx, |this, cx| {
                this.editor.saving = false;
                match res {
                    Ok(tokens) => {
                        this.editor.status = Some(("Скинуто".into(), Some(true)));
                        this.state.update(cx, |s, cx| {
                            s.tokens = Some(tokens);
                            let _ = harmoniya_api::auth::storage::save(s.tokens.as_ref().unwrap());
                            s.fetch_skin_profile(cx);
                        });
                    }
                    Err(e) => {
                        this.editor.status = Some((e.to_string(), Some(false)));
                    }
                }
                cx.notify();
            }).ok();
        }).detach();
    }
}

async fn ensure_token(tokens: auth::tokens::Tokens) -> anyhow::Result<auth::tokens::Tokens> {
    if tokens.is_access_expired() {
        if let Some(refresh) = tokens.refresh_token.clone() {
            return auth::coordinated_refresh(&refresh).await;
        }
    }
    Ok(tokens)
}

fn pick_png() -> Option<PathBuf> {
    native_dialog::DialogBuilder::file()
        .add_filter("PNG", ["png"])
        .open_single_file()
        .show()
        .ok()
        .flatten()
}

/// Truncate from the start with a leading ellipsis so the *end* of the file
/// name (extension, suffix) remains visible: "long-filename.png" → "…name.png".
fn truncate_start(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars { return s.to_string(); }
    let kept: String = s.chars().skip(count - (max_chars - 1)).collect();
    format!("…{kept}")
}

fn file_field(
    label: &'static str,
    name: String,
    on_pick: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::text_faint())
                .child(label),
        )
        .child(
            div()
                .flex()
                .items_center()
                .h(px(40.))
                .bg(Theme::bg())
                .rounded(Theme::radius_block())
                .overflow_hidden()
                .child(
                    div()
                        .id(SharedString::from(format!("pick-{label}")))
                        .flex_shrink_0()
                        .px(px(16.))
                        .h_full()
                        .flex()
                        .items_center()
                        .bg(Theme::surface_raised())
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Theme::text())
                        .cursor_pointer()
                        .hover(|s| s.bg(gpui::rgb(0x4a4850)))
                        .on_mouse_down(MouseButton::Left, on_pick)
                        .child("Вибрати"),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .px(px(14.))
                        .text_size(px(13.))
                        .text_color(Theme::text_faint())
                        .child(truncate_start(&name, 24)),
                ),
        )
        .into_any_element()
}

fn model_field(
    model: SkinModel,
    on_classic: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
    on_slim: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::text_faint())
                .child("МОДЕЛЬ РУК"),
        )
        .child(
            div()
                .flex()
                .gap(px(24.))
                .child(radio("звичайна", model == SkinModel::Classic, on_classic))
                .child(radio("тонка", model == SkinModel::Slim, on_slim)),
        )
        .into_any_element()
}

fn radio(
    label: &'static str,
    selected: bool,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    div()
        .id(SharedString::from(format!("radio-{label}")))
        .flex()
        .items_center()
        .gap(px(9.))
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, on_click)
        .child(
            div()
                .w(px(16.))
                .h(px(16.))
                .rounded_full()
                .border_2()
                .border_color(if selected { Theme::accent() } else { Theme::surface_raised() })
                .flex()
                .items_center()
                .justify_center()
                .child(if selected {
                    div().w(px(8.)).h(px(8.)).rounded_full().bg(Theme::accent()).into_any_element()
                } else {
                    div().into_any_element()
                }),
        )
        .child(
            div()
                .text_size(px(14.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if selected { Theme::text() } else { Theme::text_muted() })
                .child(label),
        )
        .into_any_element()
}

fn action_button(
    label: &'static str,
    enabled: bool,
    bg: gpui::Rgba,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    let mut btn = div()
        .id(SharedString::from(format!("action-{label}")))
        .px(px(24.))
        .py(px(10.))
        .rounded(Theme::radius_block())
        .bg(bg)
        .text_color(gpui::rgb(0x0e0d0f))
        .font_weight(FontWeight::BOLD)
        .text_size(px(14.))
        .child(label);
    if enabled {
        btn = btn.cursor_pointer().on_mouse_down(MouseButton::Left, on_click);
    } else {
        btn = btn.opacity(0.4);
    }
    btn.into_any_element()
}

fn reset_link(
    label: &'static str,
    enabled: bool,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    let mut btn = div()
        .id(SharedString::from(format!("reset-{label}")))
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(Theme::text_faint())
        .child(label);
    if enabled {
        btn = btn
            .cursor_pointer()
            .hover(|s| s.text_color(Theme::status_offline()))
            .on_mouse_down(MouseButton::Left, on_click);
    } else {
        btn = btn.opacity(0.35);
    }
    btn.into_any_element()
}
