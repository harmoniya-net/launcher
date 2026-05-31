use gpui::{
    div, px, App, Div, ElementId, InteractiveElement, MouseButton, MouseDownEvent, ParentElement,
    Stateful, Styled, Window,
};

use crate::theme::Theme;

/// A pill toggle: accent track + white knob that slides on/off. One widget for
/// every on/off switch (feature toggles, close-to-tray, …) so the pixel sizes
/// and colors aren't re-specified per call site.
pub fn toggle_switch(
    id: impl Into<ElementId>,
    on: bool,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let knob = div()
        .w(px(16.))
        .h(px(16.))
        .rounded_full()
        .bg(Theme::text())
        .ml(if on { px(20.) } else { px(2.) });
    div()
        .id(id.into())
        .w(px(38.))
        .h(px(22.))
        .rounded_full()
        .flex()
        .items_center()
        .flex_shrink_0()
        .bg(if on { Theme::accent() } else { Theme::surface_raised() })
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, on_click)
        .child(knob)
}
