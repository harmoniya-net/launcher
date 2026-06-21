//! GPUI widget wrapping the `mc_skin` software rasterizer.
//!
//! Owns the view state the rasterizer is a pure function of (loaded skin/cape
//! textures, model, yaw/pitch) plus the idle-animation clock and drag handling,
//! and feeds the rendered frame to GPUI's `img()`.
//!
//! The `mc_skin` rasterizer is a CPU software renderer, too heavy to run on the
//! UI thread every frame, so renders run on a background worker (one at a time)
//! and the finished frame is swapped in via `cx.notify`. The UI thread only ever
//! paints the last ready frame, so the view stays smooth even while the idle
//! animation or a drag keeps requesting new frames.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    div, img, px, Context, DispatchPhase, Entity, FontWeight, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point,
    Render, RenderImage, Styled, Task, Window,
};
use image::{Frame, RgbaImage};

use harmoniya_api::services::yggdrasil::SkinModel;
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
/// Idle-animation framerate. The sway is slow, so this is visually identical to
/// full vsync while cutting the repaint/present and rasterization rate (the
/// window's display runs at 60Hz+, but the animation doesn't need it). Drags
/// aren't capped by this — they repaint on each mouse-move.
const IDLE_FPS: f32 = 24.0;

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
    skin_source: Source,
    skin: Option<RgbaImage>,
    cape_source: Source,
    cape: Option<RgbaImage>,
    model: SkinModel,
    yaw: f32,
    pitch: f32,
    drag: Option<(Point<Pixels>, f32, f32)>,
    /// Last completed frame; always painted (kept while a new one renders so the
    /// view never flashes empty mid-update).
    rendered: Option<Arc<RenderImage>>,
    /// Background rasterization in flight, if any. Single-slot: we never queue
    /// more than one, so a slow worker just throttles the framerate instead of
    /// piling up work. `None` means the worker is idle.
    inflight: Option<Task<()>>,
    /// Pending paced-repaint timer. Single-shot, rescheduled from `render`, so it
    /// stops automatically when the view is unmounted (no more `render` calls).
    frame_timer: Option<Task<()>>,
    /// Set when the *next* desired frame differs from `rendered` for a reason
    /// other than the animation clock advancing (transform/skin/cape/model
    /// changed). Drives a re-render even within the same animation quantum.
    dirty: bool,
    /// Wall-clock origin for the idle animation; `t = elapsed().as_secs_f32()`.
    start: Instant,
    /// Animation `t` of the last frame we *dispatched* to the worker. Used to
    /// re-render once the animation has advanced enough to matter.
    last_t: f32,
    /// Device pixel ratio of the last dispatched render; re-render if the window
    /// moves to a display with a different scale so the output stays crisp.
    last_scale: f32,
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
                this.dirty = true;
                cx.notify();
            }
        })
        .detach();

        let s = state.read(cx);
        let initial_skin = skin_source(s);
        let initial_cape = cape_source(s);
        let model = s.current_skin_model();
        let this = Self {
            skin_source: initial_skin.clone(),
            skin: None,
            cape_source: initial_cape.clone(),
            cape: None,
            model,
            yaw: 0.0,
            pitch: 0.0,
            drag: None,
            rendered: None,
            inflight: None,
            frame_timer: None,
            dirty: false,
            start: Instant::now(),
            last_t: 0.0,
            last_scale: 1.0,
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
                    Self::decode_and_store(&this, cx, &bytes, is_skin, &key);
                })
                .detach();
            }
            Source::Url(url) => {
                let key = Source::Url(url.clone());
                cx.spawn(async move |this, cx| {
                    let bytes = match harmoniya_api::http::on_tokio(async move {
                        harmoniya_api::http::client()
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
                    Self::decode_and_store(&this, cx, &bytes, is_skin, &key);
                })
                .detach();
            }
        }
    }

    /// Decode raw image `bytes` and, if the load still matches `key` (i.e. the
    /// source wasn't superseded mid-flight), store the RGBA into the skin/cape
    /// slot and invalidate the render cache. Shared by both load paths.
    fn decode_and_store(
        this: &gpui::WeakEntity<Self>,
        cx: &mut gpui::AsyncApp,
        bytes: &[u8],
        is_skin: bool,
        key: &Source,
    ) {
        let Ok(decoded) = image::load_from_memory(bytes) else {
            return;
        };
        let rgba = decoded.to_rgba8();
        this.update(cx, |s, cx| {
            if !s.current_source(is_skin).same(key) {
                return;
            }
            if is_skin {
                s.skin = Some(rgba);
            } else {
                s.cape = Some(rgba);
            }
            s.dirty = true;
            cx.notify();
        })
        .ok();
    }

    fn current_source(&self, is_skin: bool) -> Source {
        if is_skin {
            self.skin_source.clone()
        } else {
            self.cape_source.clone()
        }
    }

    /// Kick off a background rasterization for the current state if one is
    /// warranted and none is already running. The `mc_skin` software rasterizer
    /// is too heavy to run on the UI thread every frame, so it runs on a worker
    /// and the finished frame is swapped into `rendered` via `cx.notify`. Keeping
    /// a single in-flight render means a slow worker throttles the framerate
    /// rather than queueing up stale work.
    fn maybe_spawn_render(&mut self, t: f32, scale: f32, cx: &mut Context<Self>) {
        if self.inflight.is_some() {
            return;
        }
        // Re-render when the transform/skin changed (`dirty`), nothing has been
        // drawn yet, the animation advanced past a frame quantum, or the display
        // scale changed (output resolution must follow it). The quantum is half
        // the frame interval so a paced tick reliably clears it (and an off-beat
        // repaint, e.g. an ancestor re-render, doesn't dispatch a wasted frame).
        let stale = self.dirty
            || self.rendered.is_none()
            || (t - self.last_t).abs() >= 0.5 / IDLE_FPS
            || scale != self.last_scale;
        if !stale {
            return;
        }
        let Some(skin) = self.skin.clone() else {
            return;
        };
        let cape = self.cape.clone();
        let model = render_model(self.model);
        let (yaw, pitch) = (self.yaw, self.pitch);

        self.dirty = false;
        self.last_t = t;
        self.last_scale = scale;

        self.inflight = Some(cx.spawn(async move |this, cx| {
            let frame = cx
                .background_executor()
                .spawn(async move {
                    let mut buf =
                        mc_skin::rasterize(&skin, cape.as_ref(), model, yaw, pitch, t, scale);
                    // RenderImage stores BGRA; our rasterizer produced RGBA so swap R↔B.
                    for px in buf.chunks_exact_mut(4) {
                        px.swap(0, 2);
                    }
                    buf
                })
                .await;
            this.update(cx, |s, _cx| {
                s.rendered = Some(Arc::new(RenderImage::new(vec![Frame::new(frame)])));
                s.inflight = None;
                // Deliberately no `notify`: the paced frame timer drives repaints,
                // so this frame is shown on the next tick rather than presenting
                // off-cadence (which would defeat the pacing).
            })
            .ok();
        }));
    }
}

impl Render for SkinViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.start.elapsed().as_secs_f32();
        // Render at the display's device-pixel ratio so the image paints 1:1
        // (no nearest-neighbour upscaling → no chunky pixels on HiDPI).
        let scale = window.scale_factor();
        // Dispatch the (heavy) rasterization to a worker if needed; we paint
        // whatever frame is already ready below.
        self.maybe_spawn_render(t, scale, cx);
        // Drive the idle animation at a paced rate instead of every vsync: a
        // single-shot timer re-notifies us ~IDLE_FPS times a second. Because it's
        // (re)armed here in `render`, it naturally stops when the view unmounts.
        if self.frame_timer.is_none() {
            self.frame_timer = Some(cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_secs_f32(1.0 / IDLE_FPS))
                    .await;
                this.update(cx, |s, cx| {
                    s.frame_timer = None;
                    cx.notify();
                })
                .ok();
            }));
        }

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
                            viewer.dirty = true;
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
            .child(crate::i18n::t().drag_to_rotate);

        // Always occupy the model's footprint — reserve the box before the first
        // frame renders so the label below doesn't jump when the image appears.
        if let Some(image) = &self.rendered {
            wrapper = wrapper.child(img(image.clone()).w(px(W)).h(px(H)));
        } else {
            wrapper = wrapper.child(div().w(px(W)).h(px(H)));
        }
        wrapper.child(label)
    }
}
