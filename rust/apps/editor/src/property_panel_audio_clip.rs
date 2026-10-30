use crate::event_bus::EventBus;
use crate::property_panel::{
    audio_properties_view, clip_property_panel_frame_view, clip_property_panel_header_view,
    property_panel_divider_view, property_panel_heading_view, readonly_property_panel_view,
    timing_card_view,
};
use crate::theme::{ACCENT, BORDER, MUTED, PANEL};
use crate::timeline::{FrameRate, TimelineRuntimeState};
use crate::timeline_clip::AudioClip;
use crate::timeline_document::resolve_asset_path;
use crate::track::TrackKind;
use ::waveform::WaveformData;
use gpui::FocusHandle;
use gpui::{Bounds, Entity, ParentElement, Styled, canvas, div, fill, point, px, rgb, rgba, size};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) struct AudioClipPanel {
    clip: AudioClip,
    asset_name: String,
    track_name: String,
    frame_rate: FrameRate,
    event_bus: Entity<EventBus>,
    return_focus: FocusHandle,
    locked: bool,                        // 所在轨道的锁定状态，构造面板时读取。
    waveform: Option<Arc<WaveformData>>, // 构造面板时缓存里还没有波形则为 None
}

pub(super) fn audio_clip_panel(
    timeline: &TimelineRuntimeState,
    clip: &AudioClip,
    waveform_cache: &HashMap<PathBuf, Arc<WaveformData>>,
    event_bus: Entity<EventBus>,
    return_focus: FocusHandle,
) -> AudioClipPanel {
    let asset = timeline.editing_state.asset(clip.asset_id);
    let asset_name = match asset {
        Some(asset) => asset.name().clone(),
        None => "Missing media".to_owned(),
    };
    let track_name = match timeline
        .editing_state
        .tracks
        .iter()
        .find(|track| track.id == clip.track_id)
    {
        Some(track) => track.name.clone(),
        None => "Unknown track".to_owned(),
    };
    let waveform = (|| {
        let Some(asset) = asset else {
            return None;
        };
        let Some(timeline_directory) = timeline.path.parent() else {
            return None;
        };
        let media_path = resolve_asset_path(timeline_directory, &asset.path()); // 波形缓存以素材的绝对路径为键。
        waveform_cache.get(&media_path).cloned()
    })();
    AudioClipPanel {
        clip: clip.clone(),
        asset_name,
        track_name,
        frame_rate: timeline.editing_state.settings.frame_rate,
        event_bus,
        return_focus,
        locked: timeline.editing_state.clip_locked(clip.id()),
        waveform,
    }
}

pub(super) fn audio_clip_view(panel: &AudioClipPanel) -> gpui::AnyElement {
    let clip = &panel.clip;
    let frame_rate = panel.frame_rate;
    let duration = clip.source_out - clip.source_in;
    let header = clip_property_panel_header_view(
        TrackKind::Audio,
        panel.asset_name.clone(),
        &panel.track_name,
        panel.locked,
    );
    let content = div()
        .child(property_panel_heading_view("TIMING"))
        .child(
            div()
                .mt(px(12.0))
                .grid()
                .grid_cols(2)
                .gap(px(8.0))
                .child(timing_card_view(
                    "Start",
                    clip.timeline_start,
                    frame_rate,
                    panel.locked,
                ))
                .child(timing_card_view(
                    "Duration",
                    duration,
                    frame_rate,
                    panel.locked,
                ))
                .child(timing_card_view(
                    "Source in",
                    clip.source_in,
                    frame_rate,
                    panel.locked,
                ))
                .child(timing_card_view(
                    "Source out",
                    clip.source_out,
                    frame_rate,
                    panel.locked,
                )),
        )
        .child(property_panel_divider_view())
        .child(property_panel_heading_view("AUDIO"))
        .child(div().mt(px(12.0)).child(audio_waveform_view(
            panel.waveform.clone(),
            frame_rate.seconds(clip.source_in),
            frame_rate.seconds(clip.source_out),
            panel.locked,
        )))
        .child(
            audio_properties_view(
                clip.id(),
                &clip.audio_properties,
                panel.locked,
                panel.event_bus.clone(),
                panel.return_focus.clone(),
            )
            .mt(px(16.0)),
        );
    clip_property_panel_frame_view(
        readonly_property_panel_view("audio-clip-property-panel", header, content),
        panel.locked,
    )
}

fn audio_waveform_view(
    waveform: Option<Arc<WaveformData>>,
    source_start_seconds: f64,
    source_end_seconds: f64,
    locked: bool,
) -> gpui::Div {
    div()
        .h(px(44.0))
        .overflow_hidden()
        .rounded(px(6.0))
        .bg(rgb(if locked { 0x19191d } else { 0x131316 }))
        .child(
            canvas(
                move |bounds, _, _| match &waveform {
                    Some(waveform) => waveform.columns(
                        source_start_seconds,
                        source_end_seconds,
                        (f32::from(bounds.size.width) / 3.0).floor().max(1.0) as usize, // 每根柱 2px 宽、间隔 1px
                    ),
                    None => Vec::new(),
                },
                |bounds, columns, window, _| {
                    let height = f32::from(bounds.size.height);
                    for (index, peak) in columns.iter().enumerate() {
                        let bar_height =
                            (peak.max.abs().max(peak.min.abs()) * height).clamp(1.0, height);
                        window.paint_quad(fill(
                            Bounds::new(
                                point(
                                    bounds.left() + px(index as f32 * 3.0),
                                    bounds.top() + px((height - bar_height) / 2.0),
                                ),
                                size(px(2.0), px(bar_height)),
                            ),
                            rgba(0x69c5cfcc),
                        ));
                    }
                },
            )
            .size_full(),
        )
}
