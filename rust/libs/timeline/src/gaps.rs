use crate::{TimelineEditingState, TimelineFrameIndex};

pub fn remove_gaps(editing_state: &TimelineEditingState) -> TimelineEditingState {
    let frame_rate = editing_state.settings.frame_rate;
    let mut ordered_clips = editing_state.clips.iter().enumerate().collect::<Vec<_>>();
    ordered_clips.sort_unstable_by_key(|(_, clip)| clip.timeline_start());
    let mut result = editing_state.clone();
    let mut covered_end = TimelineFrameIndex::ZERO; // 原时间线已遍历片段的最远终点。
    let mut removed_frames = TimelineFrameIndex::ZERO; // 当前片段之前累计删除的空隙长度。
    for (index, clip) in ordered_clips {
        let start = clip.timeline_start();
        if start > covered_end {
            removed_frames += start - covered_end;
        }
        result.clips[index].set_timeline_start(start - removed_frames);
        covered_end = covered_end.max(clip.timeline_end(frame_rate));
    }
    result
}
