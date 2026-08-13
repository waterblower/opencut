use super::*;
use std::path::PathBuf;

pub(super) fn preview_image_file(path: PathBuf, width: f32, height: f32) -> gpui::AnyElement {
    div()
        .id("editor-image-file-preview")
        .w(px(width))
        .h(px(height))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .bg(rgb(0x000000))
        .child(img(path).size_full().object_fit(ObjectFit::Contain))
        .into_any_element()
}
