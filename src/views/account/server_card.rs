use std::sync::Arc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, FontWeight, Image, ImageSource, InteractiveElement,
    IntoElement, MouseButton, ObjectFit, ParentElement, SharedString, StatefulInteractiveElement,
    StyledImage, Styled, Window, div, ease_in_out, hsla, img, linear_color_stop, linear_gradient,
    prelude::FluentBuilder, px, relative, rgb,
};

use harmoniya_api::services::modpacks::Modpack;
use crate::theme::Theme;

/// `prev_h` / `target_h`: the height to tween from/to. Caller (ServerList)
/// computes these based on the card's role in its group (active/neighbor/edge)
/// and tracks the last rendered height so the animation always starts from the
/// real current position rather than a hardcoded inverse.
pub fn server_card(
    m: Modpack,
    active: bool,
    hovered: bool,
    prev_h: f32,
    target_h: f32,
    banner: Option<Arc<Image>>,
    on_select: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
    on_hover: impl Fn(&bool, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let id = m.id.clone();
    let status = derive_status(&m);
    let banner_url = m
        .banner
        .as_ref()
        .and_then(|b| b.url.as_deref())
        .map(|u| crate::banner::at_size(u, 816, 400));

    let group_name = SharedString::from(format!("card-{id}"));

    let text_color = if active || hovered { Theme::text() } else { Theme::text_faint() };
    let mut card = div()
        .id(("server-card", hash_id(&id)))
        .group(group_name.clone())
        .relative()
        .flex()
        .flex_col()
        .justify_between()
        .flex_shrink_0()
        .w_full()
        .px(px(20.))
        .py(px(16.))
        .rounded(Theme::radius_card())
        .bg(if active { Theme::surface_raised() } else { Theme::surface() })
        .text_color(text_color)
        .cursor_pointer()
        .overflow_hidden()
        .on_hover(on_hover)
        .on_mouse_down(MouseButton::Left, on_select);

    if let Some(url) = banner_url {
        // Prefer pre-fetched bytes if available; otherwise hand GPUI the URL
        // (which kicks off the slow network path on first paint).
        let source: ImageSource = match banner {
            Some(arc) => arc.into(),
            None => url.into(),
        };
        let mut banner_img = img(source)
            .object_fit(ObjectFit::Cover)
            .absolute()
            .inset_0()
            .size_full()
            .rounded(Theme::radius_card())
            .opacity(if active { 0.85 } else { 0.35 });
        if !active {
            banner_img = banner_img.group_hover(group_name.clone(), |s| s.opacity(0.85));
        }
        card = card.child(banner_img);
        // Inner shadow from top and bottom: two gradient bands that fade dark
        // into transparent toward the card's vertical center. Each band covers
        // 60% of the card so they overlap slightly in the middle for a smoother
        // transition. Alpha switches via group_hover so the shadow lightens.
        let top_alpha = if active || hovered { 0.80 } else { 0.85 };
        let bottom_alpha = if active {
            0.20
        } else if hovered {
            0.45
        } else {
            0.85
        };
        let dark = hsla(0.0, 0.0, 0.0, 1.0);
        let band_h = (target_h * 0.9).max(80.0);

        let top_band = div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(band_h))
            .bg(linear_gradient(
                180.0,
                linear_color_stop(dark, 0.0).opacity(top_alpha),
                linear_color_stop(dark, 1.0).opacity(0.0),
            ));

        let bottom_band = div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .h(px(band_h))
            .bg(linear_gradient(
                0.0,
                linear_color_stop(dark, 0.0).opacity(bottom_alpha),
                linear_color_stop(dark, 1.0).opacity(0.0),
            ));

        card = card.child(top_band).child(bottom_band);
    }

    let built = card
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(16.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(text_color)
                                .child(m.title.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(text_color)
                                .child(m.version.unwrap_or_default()),
                        ),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(text_color)
                        .child(m.summary.unwrap_or_default()),
                ),
        )
        .child(
            div()
                .relative()
                .flex()
                .justify_end()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(Theme::radius_block())
                        .bg(rgb(0x0e0d0f))
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Theme::text_muted())
                                .child(status.text),
                        )
                        .child(
                            div()
                                .w(px(7.))
                                .h(px(7.))
                                .rounded_full()
                                .bg(status.color),
                        ),
                ),
        );

    // Animation key encodes the target height so any change in role (active /
    // neighbor / normal / edge-active) re-fires the tween, interpolating from
    // the previous render's actual height to the new target.
    let anim_id = SharedString::from(format!("card-h-{id}-{target_h}"));
    built
        .with_animation(
            anim_id,
            Animation::new(Duration::from_millis(140)).with_easing(ease_in_out),
            move |this, t| this.h(px(prev_h + (target_h - prev_h) * t)),
        )
        .into_any_element()
}

struct ResolvedStatus {
    text: String,
    color: gpui::Rgba,
}

fn derive_status(m: &Modpack) -> ResolvedStatus {
    if m.maintaining {
        return ResolvedStatus { text: "Тех. Роботи".into(), color: Theme::status_maintenance() };
    }
    match &m.status {
        None => ResolvedStatus { text: "Офлайн".into(), color: Theme::status_offline() },
        Some(s) => ResolvedStatus { text: s.online.to_string(), color: Theme::status_online() },
    }
}

fn hash_id(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}
