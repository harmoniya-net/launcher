use gpui::{AnyElement, Hsla, IntoElement, ParentElement, Styled, div, px, svg};
use rsx::rsx;

/// Render an SVG icon at a fixed size. Wrapping in a div ensures the layout
/// engine always gives it definite pixel bounds before the SVG renderer runs.
pub fn icon(path: &'static str, size: f32, color: impl Into<Hsla>) -> AnyElement {
    let color: Hsla = color.into();
    rsx! {
        <div w={px(size)} h={px(size)} flex_shrink_0>
            <svg path={path} text_color={color} w={px(size)} h={px(size)} />
        </div>
    }
    .into_any_element()
}
