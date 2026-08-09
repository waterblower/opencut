use super::*;
use std::path::Path;

impl Editor {
    pub(super) fn preview_image_file(
        &self,
        path: &Path,
        width: f32,
        height: f32,
    ) -> gpui::AnyElement {
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
            .child(
                img(self.project_root.join(path))
                    .size_full()
                    .object_fit(ObjectFit::Contain),
            )
            .into_any_element()
    }
}
