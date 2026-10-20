use crate::property_panel::{
    file_badge_view, file_details_view, file_hint_view, file_location_view,
    property_panel_divider_view, property_panel_header_view, property_panel_heading_view,
    readonly_property_panel_view,
};
use gpui::{ParentElement, div};
use std::path::PathBuf;

pub(crate) struct VideoFilePanel {
    pub(crate) path: PathBuf,           // 绝对路径
    pub(crate) size_bytes: Option<u64>, // 取自 Explorer 条目，不在 render 里读磁盘
}

pub(super) fn video_file_view(panel: &VideoFilePanel) -> gpui::AnyElement {
    let path = panel.path.as_path();
    let name = match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.display().to_string(),
    };
    let format = match path.extension() {
        Some(extension) => extension.to_string_lossy().to_ascii_uppercase(),
        None => "Unknown".to_owned(),
    };
    let header = property_panel_header_view(
        file_badge_view(&format, 0x355b78, 0x8fb9dd),
        name,
        "Video file".to_owned(),
    );
    let content = div()
        .child(property_panel_heading_view("FILE"))
        .child(file_details_view(path, panel.size_bytes))
        .child(property_panel_divider_view())
        .child(property_panel_heading_view("LOCATION"))
        .child(file_location_view(path))
        .child(property_panel_divider_view())
        .child(file_hint_view(
            "Drag onto a video track to add it to the timeline.",
        ));
    readonly_property_panel_view("video-file-property-panel", header, content)
}
