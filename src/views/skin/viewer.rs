//! GPUI widget wrapping the `mc_skin` software rasterizer.
//!
//! Owns the view state the rasterizer is a pure function of (loaded skin/cape
//! textures, model, yaw/pitch) plus the idle-animation clock and drag handling,
//! and feeds the rendered frame to GPUI's `img()`. Re-renders happen only when
//! the transform or animation actually advances, so cost shows up while
//! dragging or animating, not at rest.

use std::sync::Arc;
use std::time::Instant;

use gpui::{
    div, img, px, Context, DispatchPhase, Entity, FontWeight, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point,
    Render, RenderImage, Styled, Window,
};
use image::{Frame, RgbaImage};

use crate::services::yggdrasil::SkinModel;
use crate::state::AppState;
use crate::theme::Theme;

/// Output footprint (matches the rasterizer's fixed render size).
const W: f32 = mc_skin::WIDTH as f32;
const H: f32 = mc_skin::HEIGHT as f32;
/// The wrapper extends past the rendered image to give the user a larger
/// grab zone for rotation drags.
const DRAG_PAD_X: f32 = 40.;
const DRAG_PAD_Y: f32 = 60.;
const DRAG_YAW_PER_PX: f32 = 0.01;
const DRAG_PITCH_PER_PX: f32 = 0.006;
const PITCH_LIMIT: f32 = 0.6; // ~34°

/// Map the account-service skin model onto the renderer's arm model.
fn render_model(model: SkinModel) -> mc_skin::Model {
    match model {
        SkinModel::Slim => mc_skin::Model::Slim,
        SkinModel::Classic => mc_skin::Model::Classic,
    }
}

#[derive(Clone, Default)]
enum Source {
    #[default]
    None,
    /// Local file preview (decoded synchronously from in-memory bytes).
    Preview(Arc<Vec<u8>>),
    /// Remote URL (fetched asynchronously).
    Url(String),
}

impl Source {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Source::None, Source::None) => true,
            (Source::Preview(a), Source::Preview(b)) => Arc::ptr_eq(a, b),
            (Source::Url(a), Source::Url(b)) => a == b,
            _ => false,
        }
    }
}

fn skin_source(s: &AppState) -> Source {
    if let Some(b) = &s.preview_skin_bytes {
        return Source::Preview(b.clone());
    }
    if let Some(u) = s.skin_profile.as_ref().and_then(|p| p.skin_url.clone()) {
        return Source::Url(u);
    }
    Source::None
}

fn cape_source(s: &AppState) -> Source {
    if let Some(b) = &s.preview_cape_bytes {
        return Source::Preview(b.clone());
    }
    if let Some(u) = s.skin_profile.as_ref().and_then(|p| p.cape_url.clone()) {
        return Source::Url(u);
    }
    Source::None
}

pub struct SkinViewer {
    state: Entity<AppState>,
    skin_source: Source,
    skin: Option<RgbaImage>,
    cape_source: Source,
    cape: Option<RgbaImage>,
    model: SkinModel,
    yaw: f32,
    pitch: f32,
    drag: Option<(Point<Pixels>, f32, f32)>,
    rendered: Option<Arc<RenderImage>>,
    /// Wall-clock origin for the idle animation; `t = elapsed().as_secs_f32()`.
    start: Instant,
    /// Last animation `t` we rendered for. Used to invalidate the cache when
    /// the animation has advanced enough to matter.
    last_t: f32,
}

impl SkinViewer {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |this, st, cx| {
            let s = st.read(cx);
            let desired_skin = skin_source(s);
            let desired_cape = cape_source(s);
            let model = s.current_skin_model();
            let mut changed = false;
            if !desired_skin.same(&this.skin_source) {
                this.skin_source = desired_skin.clone();
                this.skin = None;
                Self::load_source(desired_skin, true, cx);
                changed = true;
            }
            if !desired_cape.same(&this.cape_source) {
                this.cape_source = desired_cape.clone();
                this.cape = None;
                Self::load_source(desired_cape, false, cx);
                changed = true;
            }
            if model != this.model {
                this.model = model;
                changed = true;
            }
            if changed {
                this.rendered = None;
                cx.notify();
            }
        })
        .detach();

        let s = state.read(cx);
        let initial_skin = skin_source(s);
        let initial_cape = cape_source(s);
        let model = s.current_skin_model();
        let this = Self {
            state: state.clone(),
            skin_source: initial_skin.clone(),
            skin: None,
            cape_source: initial_cape.clone(),
            cape: None,
            model,
            yaw: 0.0,
            pitch: 0.0,
            drag: None,
            rendered: None,
            start: Instant::now(),
            last_t: 0.0,
        };
        Self::load_source(initial_skin, true, cx);
        Self::load_source(initial_cape, false, cx);
        this
    }

    fn load_source(source: Source, is_skin: bool, cx: &mut Context<Self>) {
        match source {
            Source::None => {}
            Source::Preview(bytes) => {
                let key = Source::Preview(bytes.clone());
                cx.spawn(async move |this, cx| {
                    let Ok(decoded) = image::load_from_memory(&bytes) else {
                        return;
                    };
                    let rgba = decoded.to_rgba8();
                    this.update(cx, |s, cx| {
                        if !s.current_source(is_skin).same(&key) {
                            return;
                        }
                        if is_skin {
                            s.skin = Some(rgba);
                        } else {
                            s.cape = Some(rgba);
                        }
                        s.rendered = None;
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
            Source::Url(url) => {
                let key = Source::Url(url.clone());
                cx.spawn(async move |this, cx| {
                    let bytes = match crate::http::on_tokio(async move {
                        crate::http::client()
                            .get(&url)
                            .send()
                            .await?
                            .error_for_status()?
                            .bytes()
                            .await
                    })
                    .await
                    {
                        Ok(b) => b.to_vec(),
                        Err(_) => return,
                    };
                    let Ok(decoded) = image::load_from_memory(&bytes) else {
                        return;
                    };
                    let rgba = decoded.to_rgba8();
                    this.update(cx, |s, cx| {
                        if !s.current_source(is_skin).same(&key) {
                            return;
                        }
                        if is_skin {
                            s.skin = Some(rgba);
                        } else {
                            s.cape = Some(rgba);
                        }
                        s.rendered = None;
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
        }
    }

    fn current_source(&self, is_skin: bool) -> Source {
        if is_skin {
            self.skin_source.clone()
        } else {
            self.cape_source.clone()
        }
    }

    fn ensure_render(&mut self, t: f32, cx: &mut Context<Self>) -> Option<Arc<RenderImage>> {
        // Invalidate cache when the idle animation has advanced enough that a
        // re-render visibly changes anything (~30 fps quantum).
        if (t - self.last_t).abs() >= 1.0 / 30.0 {
            self.rendered = None;
        }
        if let Some(r) = &self.rendered {
            return Some(r.clone());
        }
        let skin = self.skin.as_ref()?;
        let model = render_model(self.state.read(cx).current_skin_model());
        let mut buf = mc_skin::rasterize(skin, self.cape.as_ref(), model, self.yaw, self.pitch, t);
        // RenderImage stores BGRA; our rasterizer produced RGBA so swap R↔B.
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        let arc = Arc::new(RenderImage::new(vec![Frame::new(buf)]));
        self.rendered = Some(arc.clone());
        self.last_t = t;
        Some(arc)
    }
}

impl Render for SkinViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.start.elapsed().as_secs_f32();
        let rendered = self.ensure_render(t, cx);
        // Drive the idle animation: schedule another paint at the next vsync.
        window.request_animation_frame();

        // While a drag is active, capture mouse-move and mouse-up at the
        // window level so the rotation keeps tracking even when the cursor
        // leaves the viewer's hitbox.
        if self.drag.is_some() {
            let this = cx.entity();
            window.on_mouse_event({
                let this = this.clone();
                move |e: &MouseMoveEvent, phase, _window, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    this.update(cx, |viewer, cx| {
                        let Some((start, base_yaw, base_pitch)) = viewer.drag else {
                            return;
                        };
                        if e.pressed_button != Some(MouseButton::Left) {
                            viewer.drag = None;
                            return;
                        }
                        let dx: f32 = (e.position.x - start.x).into();
                        let dy: f32 = (e.position.y - start.y).into();
                        let new_yaw = base_yaw + dx * DRAG_YAW_PER_PX;
                        let new_pitch =
                            (base_pitch + dy * DRAG_PITCH_PER_PX).clamp(-PITCH_LIMIT, PITCH_LIMIT);
                        if (new_yaw - viewer.yaw).abs() > 1e-4
                            || (new_pitch - viewer.pitch).abs() > 1e-4
                        {
                            viewer.yaw = new_yaw;
                            viewer.pitch = new_pitch;
                            viewer.rendered = None;
                            cx.notify();
                        }
                    });
                }
            });
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                this.update(cx, |viewer, cx| {
                    viewer.drag = None;
                    cx.notify();
                });
            });
        }

        let mut wrapper = div()
            .id("skin-viewer")
            .w(px(W + DRAG_PAD_X))
            .h(px(H + DRAG_PAD_Y))
            .flex()
            .flex_col()
            .items_center()
            .mt(px(32.))
            .gap(px(8.))
            .cursor(gpui::CursorStyle::OpenHand);

        wrapper = wrapper.on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, e: &MouseDownEvent, _, cx| {
                this.drag = Some((e.position, this.yaw, this.pitch));
                cx.notify();
            }),
        );

        let label = div()
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(Theme::text_faint())
            .child("Перетягни для обертання");

        // Always occupy the model's footprint — reserve the box before the first
        // frame renders so the label below doesn't jump when the image appears.
        if let Some(image) = rendered {
            wrapper = wrapper.child(img(image).w(px(W)).h(px(H)));
        } else {
            wrapper = wrapper.child(div().w(px(W)).h(px(H)));
        }
        wrapper.child(label)
    }
}
