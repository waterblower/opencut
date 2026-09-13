use crate::{
    cli::error::{Error, Result},
    cli_error,
    timeline::{Clip, MediaKind, TimelineSerialization, TimelineTime, TrackKind},
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use ulid::Ulid;

#[derive(Debug)]
pub struct Finding {
    pub error: Error,
    pub fix_hint: Option<String>,
}

impl Serialize for Finding {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Finding", 2)?;
        state.serialize_field("message", &format!("{:#}", self.error))?;
        state.serialize_field("fix_hint", &self.fix_hint)?;
        state.end()
    }
}

#[derive(Clone, Debug)]
pub struct MediaInfo {
    pub duration: f64,
    pub video: bool,
    pub audio: bool,
    pub image: bool,
    pub video_bitrate: Option<u64>,
}

pub fn validate(
    doc: &TimelineSerialization,
    media: Option<&HashMap<Ulid, MediaInfo>>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    macro_rules! check {
        ($condition:expr, $code:expr, $pointer:expr, $message:expr) => {
            if !$condition {
                findings.push(Finding {
                    error: cli_error!($code, &$pointer, 3, "{}", $message),
                    fix_hint: None,
                });
            }
        };
    }
    let settings = &doc.settings;
    let fps = settings.frame_rate;
    check!(
        (2..=16384).contains(&settings.width),
        "invalid_dimensions",
        "/settings/width",
        "width must be 2..16384"
    );
    check!(
        (2..=16384).contains(&settings.height),
        "invalid_dimensions",
        "/settings/height",
        "height must be 2..16384"
    );
    check!(
        fps.numerator > 0
            && fps.denominator > 0
            && fps.numerator <= i32::MAX as u32
            && fps.denominator <= i32::MAX as u32,
        "invalid_frame_rate",
        "/settings/frame_rate",
        "frame rate components must be positive signed 32-bit integers"
    );
    check!(
        settings.audio_sample_rate >= 8000,
        "invalid_sample_rate",
        "/settings/audio_sample_rate",
        "sample rate must be at least 8000"
    );
    let mut ids = HashSet::new();
    for (index, asset) in doc.assets.iter().enumerate() {
        check!(
            !asset.id.is_nil() && ids.insert(asset.id),
            "duplicate_id",
            format!("/assets/{index}/id"),
            "IDs must be nonzero and unique"
        );
        check!(
            !asset.path.as_os_str().is_empty(),
            "invalid_path",
            format!("/assets/{index}/path"),
            "asset path cannot be empty"
        );
        check!(
            asset.duration.is_finite() && asset.duration >= 0.0,
            "invalid_duration",
            format!("/assets/{index}/duration"),
            "asset duration must be finite and nonnegative"
        );
    }
    for (index, track) in doc.tracks.iter().enumerate() {
        check!(
            !track.id.is_nil() && ids.insert(track.id),
            "duplicate_id",
            format!("/tracks/{index}/id"),
            "IDs must be nonzero and unique"
        );
    }
    for (index, clip) in doc.clips.iter().enumerate() {
        let pointer = format!("/clips/{index}/data");
        check!(
            !clip.id().is_nil() && ids.insert(clip.id()),
            "duplicate_id",
            format!("{pointer}/id"),
            "IDs must be nonzero and unique"
        );
        check!(
            clip.timeline_start() >= TimelineTime::ZERO,
            "invalid_time",
            format!("{pointer}/timeline_start"),
            "start must be nonnegative"
        );
        let length = clip.frame_length(fps);
        check!(
            length > TimelineTime::ZERO
                && clip
                    .timeline_start()
                    .frames()
                    .checked_add(length.frames())
                    .is_some(),
            "invalid_duration",
            &pointer,
            "clip duration must be positive and representable"
        );
        let Some(track) = doc.track(clip.track_id()) else {
            check!(
                false,
                "unknown_track",
                format!("{pointer}/track_id"),
                "clip references an unknown track"
            );
            continue;
        };
        check!(
            matches!(
                (track.kind, clip),
                (TrackKind::Video, Clip::Video(_))
                    | (TrackKind::Audio, Clip::Audio(_))
                    | (TrackKind::Text, Clip::Text(_))
            ),
            "invalid_track",
            format!("{pointer}/track_id"),
            "clip kind must match its track"
        );
        match clip {
            Clip::Video(data) | Clip::Audio(data) => {
                check!(
                    data.source_in >= TimelineTime::ZERO && data.source_out > data.source_in,
                    "invalid_trim",
                    &pointer,
                    "source range must be nonnegative and nonempty"
                );
                let video = data.video_properties;
                check!(
                    video.position_x.is_finite()
                        && video.position_y.is_finite()
                        && video.scale.is_finite()
                        && video.scale >= 0.0,
                    "invalid_property",
                    format!("{pointer}/video_properties"),
                    "video transform must be finite with nonnegative scale"
                );
                check!(
                    data.audio_properties.gain_db.is_finite(),
                    "invalid_property",
                    format!("{pointer}/audio_properties/gain_db"),
                    "gain must be finite"
                );
                let Some(asset) = doc.asset(data.asset_id) else {
                    check!(
                        false,
                        "unknown_asset",
                        format!("{pointer}/asset_id"),
                        "clip references an unknown asset"
                    );
                    continue;
                };
                check!(
                    track.kind != TrackKind::Video || asset.kind != MediaKind::Audio,
                    "invalid_track",
                    &pointer,
                    "audio assets cannot be placed on a video track"
                );
                let Some(media) = media else {
                    continue;
                };
                let Some(info) = media.get(&data.asset_id) else {
                    continue;
                };
                check!(
                    asset.kind == MediaKind::Image || data.source_out <= fps.ceil(info.duration),
                    "source_bounds",
                    format!("{pointer}/source_out"),
                    "source range exceeds the media duration"
                );
                check!(
                    track.kind != TrackKind::Video || info.video || info.image,
                    "missing_video",
                    &pointer,
                    "video clip requires visual media"
                );
                check!(
                    track.kind != TrackKind::Audio || info.audio,
                    "missing_audio",
                    &pointer,
                    "audio clip requires an audio stream"
                );
            }
            Clip::Text(text) => {
                let properties = &text.properties;
                check!(
                    properties.font_size.is_finite()
                        && properties.font_size > 0.0
                        && properties.position_x.is_finite()
                        && properties.position_y.is_finite(),
                    "invalid_property",
                    format!("{pointer}/properties"),
                    "text size and position must be finite with positive size"
                );
            }
        }
    }
    for track in &doc.tracks {
        let mut clips: Vec<_> = doc
            .clips
            .iter()
            .enumerate()
            .filter(|(_, clip)| clip.track_id() == track.id)
            .collect();
        clips.sort_by_key(|(_, clip)| clip.timeline_start());
        let mut end = TimelineTime::ZERO;
        for (index, clip) in clips {
            check!(
                clip.timeline_start() >= end,
                "overlap",
                format!("/clips/{index}/data/timeline_start"),
                "clips on the same track cannot overlap"
            );
            end = end.max(clip.timeline_end(fps));
        }
    }
    findings
}

pub fn require_valid(
    doc: &TimelineSerialization,
    media: Option<&HashMap<Ulid, MediaInfo>>,
) -> Result<()> {
    let Some(finding) = validate(doc, media).into_iter().next() else {
        return Ok(());
    };
    Err(finding.error)
}
