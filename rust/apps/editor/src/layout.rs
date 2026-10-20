pub(crate) const DEFAULT_MEDIA_PANEL_WIDTH: f32 = 340.0;
pub(crate) const DEFAULT_PROPERTY_PANEL_WIDTH: f32 = 280.0;
pub(crate) const MIN_MEDIA_PANEL_WIDTH: f32 = 220.0;
pub(crate) const MIN_PROPERTY_PANEL_WIDTH: f32 = 240.0;
pub(crate) const MIN_PREVIEW_WIDTH: f32 = 320.0;
pub(crate) const TOPBAR_HEIGHT: f32 = 64.0;
pub(crate) const DEFAULT_TIMELINE_HEIGHT: f32 = 280.0;
pub(crate) const MIN_UPPER_WORKSPACE_HEIGHT: f32 = 240.0;
pub(crate) const MIN_TIMELINE_HEIGHT: f32 = TIMELINE_HEADER_HEIGHT + RULER_HEIGHT + TRACK_HEIGHT; // 至少露出一条轨道。
pub(crate) const TIMELINE_HEADER_HEIGHT: f32 = 46.0;
pub(crate) const TIMELINE_PADDING: f32 = 20.0;
pub(crate) const TRACK_HEADER_WIDTH: f32 = 190.0;
pub(crate) const TRACK_HEIGHT: f32 = 74.0;
pub(crate) const RULER_HEIGHT: f32 = 28.0;
pub(crate) const SNAP_DISTANCE_PX: f32 = 8.0;
pub(crate) const MIN_TIMELINE_PIXELS_PER_SECOND: f32 = 0.1;
pub(crate) const MAX_TIMELINE_PIXELS_PER_SECOND: f32 = 1000.0;
pub(crate) const DEFAULT_TIMELINE_PIXELS_PER_SECOND: f32 = 72.0;

/// 将指针纵坐标换算为从 0 开始的轨道索引；两个坐标须使用同一坐标系，
/// 第一条轨道的顶部坐标须包含滚动偏移，轨道高度须大于 0。
pub(crate) fn track_index_at_y(pointer_y: f32, first_track_top: f32, track_height: f32) -> isize {
    ((pointer_y - first_track_top) / track_height).floor() as isize
}
