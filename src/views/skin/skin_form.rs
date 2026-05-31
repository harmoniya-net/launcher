use std::path::PathBuf;

use gpui::{
    Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use harmoniya_api::auth::tokens::Tokens;
use harmoniya_api::services::yggdrasil::{self, SkinModel};
use crate::state::AppState;
use crate::theme::Theme;
use super::skin_form_widgets::{
    action_button, file_field, model_field, pick_png, reset_link,
};

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
        crate::views::observe_repaint(&state, cx);
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
            let res: anyhow::Result<Tokens> = harmoniya_api::http::on_tokio(async move {
                let (access, _) = harmoniya_api::auth::ensure_fresh(tokens).await?;
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
                            s.adopt_tokens(Some(tokens));
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
            let res: anyhow::Result<Tokens> = harmoniya_api::http::on_tokio(async move {
                let (access, _) = harmoniya_api::auth::ensure_fresh(tokens).await?;
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
                            s.adopt_tokens(Some(tokens));
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
