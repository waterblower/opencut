use crate::{
    cli_error,
    core::{
        document::*,
        error::{Error, Result},
    },
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize)]
pub struct Finding {
    #[serde(flatten)]
    pub error: Error,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_hint: Option<String>,
}

#[derive(Clone, Debug)]
pub struct MediaInfo {
    pub duration: f64,
    pub video: bool,
    pub audio: bool,
    pub image: bool,
    pub video_bitrate: Option<u64>,
}

pub fn validate(doc: &Document, media: Option<&HashMap<String, MediaInfo>>) -> Vec<Finding> {
    let mut findings = Vec::new();
    macro_rules! check {
        ($condition:expr, $code:expr, $pointer:expr, $message:expr) => {
            let valid = $condition;
            if !valid {
                findings.push(Finding {
                    error: cli_error!($code, &$pointer, 3, "{}", $message),
                    fix_hint: None,
                });
            }
        };
    }
    let s = &doc.settings;
    let fps = s.frame_rate;
    check!(
        doc.version == 1,
        "unsupported_version",
        "/version",
        "expected version 1"
    );
    check!(
        s.width > 0 && s.width <= 16384,
        "invalid_dimensions",
        "/settings/width",
        "width must be 1..16384"
    );
    check!(
        s.height > 0 && s.height <= 16384,
        "invalid_dimensions",
        "/settings/height",
        "height must be 1..16384"
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
        [
            8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000, 64000, 88200, 96000
        ]
        .contains(&s.sample_rate),
        "invalid_sample_rate",
        "/settings/sample_rate",
        "unsupported AAC sample rate"
    );
    check!(
        color(&s.background).is_some(),
        "invalid_color",
        "/settings/background",
        "expected #RRGGBB or #RRGGBBAA"
    );
    let mut ids = HashSet::new();
    for (kind, entities) in [
        (
            "assets",
            doc.assets.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
        ),
        ("tracks", doc.tracks.iter().map(|t| t.id.as_str()).collect()),
        (
            "clips",
            doc.clips.iter().map(|c| c.common().id.as_str()).collect(),
        ),
        (
            "transitions",
            doc.transitions.iter().map(|t| t.id.as_str()).collect(),
        ),
    ] {
        for (i, id) in entities.iter().enumerate() {
            check!(
                !id.is_empty() && ids.insert(*id),
                "duplicate_id",
                format!("/{kind}/{i}/id"),
                "IDs must be nonempty and unique across the document"
            );
        }
    }
    for (i, clip) in doc.clips.iter().enumerate() {
        let c = clip.common();
        let p = format!("/clips/{i}");
        check!(
            c.timeline_start >= 0,
            "invalid_time",
            format!("{p}/timeline_start"),
            "start must be nonnegative"
        );
        let length = clip.length(fps);
        check!(
            length > 0 && c.timeline_start.checked_add(length).is_some(),
            "invalid_duration",
            p,
            "clip must have a positive, representable duration"
        );
        check!(
            c.opacity.is_finite() && (0.0..=1.0).contains(&c.opacity),
            "invalid_property",
            format!("{p}/opacity"),
            "opacity must be 0..1"
        );
        let track = doc.tracks.iter().find(|t| t.id == c.track_id);
        check!(
            track.is_some(),
            "unknown_track",
            format!("{p}/track_id"),
            "track does not exist"
        );
        if let Some(track) = track {
            check!(
                match clip {
                    Clip::Text { .. } => track.kind != TrackKind::Audio,
                    Clip::Image { .. } => track.kind == TrackKind::Video,
                    Clip::Media { .. } => track.kind != TrackKind::Text,
                },
                "track_kind_mismatch",
                format!("{p}/track_id"),
                "clip is incompatible with track kind"
            );
        }
        let v = clip.video();
        check!(
            v.position_x.is_finite()
                && (0.0..=1.0).contains(&v.position_x)
                && v.position_y.is_finite()
                && (0.0..=1.0).contains(&v.position_y)
                && v.scale.is_finite()
                && v.scale > 0.0
                && v.scale <= 32.0,
            "invalid_transform",
            p,
            "positions must be 0..1 and scale must be positive, at most 32"
        );
        if let Clip::Text {
            properties, length, ..
        } = clip
        {
            check!(
                properties.font_size.is_finite()
                    && properties.font_size > 0.0
                    && properties.font_size <= 4096.0,
                "invalid_property",
                format!("{p}/properties/font_size"),
                "font size must be positive, at most 4096"
            );
            if let Length::Duration { nanos, .. } = length {
                check!(
                    *nanos < 1_000_000_000,
                    "invalid_duration",
                    format!("{p}/length/nanos"),
                    "nanos must be less than one billion"
                );
            }
        }
        if let Clip::Media {
            source_in,
            source_out,
            audio_properties,
            ..
        } = clip
        {
            check!(
                *source_in >= 0 && source_out > source_in,
                "invalid_trim",
                p,
                "require 0 <= source_in < source_out"
            );
            check!(
                audio_properties.gain_db.is_finite()
                    && (-120.0..=60.0).contains(&audio_properties.gain_db),
                "invalid_property",
                format!("{p}/audio_properties/gain_db"),
                "gain must be -120..60 dB"
            );
        }
        if let Some(id) = clip.asset_id() {
            check!(
                doc.assets.iter().any(|a| a.id == id),
                "unknown_asset",
                format!("{p}/asset_id"),
                "asset does not exist"
            );
            if let Some(info) = media.and_then(|m| m.get(id)) {
                if let Clip::Media { source_out, .. } = clip {
                    if fps.numerator > 0
                        && fps.denominator > 0
                        && fps.seconds(*source_out) > info.duration + 0.000001
                    {
                        findings.push(Finding {
                            error: cli_error!(
                                "clip_out_of_range",
                                &format!("{p}/source_out"),
                                3,
                                "trim exceeds source duration {}s",
                                info.duration
                            ),
                            fix_hint: Some(format!(
                                "reduce source_out to <= {}",
                                (info.duration * fps.numerator as f64 / fps.denominator as f64)
                                    .floor() as i64
                            )),
                        });
                    }
                    if let Some(track) = track {
                        check!(
                            if track.kind == TrackKind::Audio {
                                info.audio
                            } else {
                                info.video && !info.image
                            },
                            "media_kind_mismatch",
                            format!("{p}/asset_id"),
                            "asset lacks required media stream"
                        );
                    }
                }
                if matches!(clip, Clip::Image { .. }) {
                    check!(
                        info.image,
                        "media_kind_mismatch",
                        format!("{p}/asset_id"),
                        "image clip requires a still image or SVG"
                    );
                }
            }
        }
        for (j, other) in doc.clips[..i].iter().enumerate() {
            if c.track_id == other.common().track_id {
                check!(
                    c.timeline_start >= other.end(fps)
                        || other.common().timeline_start >= clip.end(fps),
                    "clip_overlap",
                    p,
                    format!("overlaps /clips/{j}")
                );
            }
        }
        for (j, effect) in c.effects.iter().enumerate() {
            let valid = match effect {
                Effect::GaussianBlur { radius } => {
                    radius.is_finite() && (0.0..=256.0).contains(radius)
                }
                Effect::ColorAdjust {
                    brightness,
                    contrast,
                    saturation,
                } => {
                    brightness.is_finite()
                        && (-1.0..=1.0).contains(brightness)
                        && contrast.is_finite()
                        && (0.0..=10.0).contains(contrast)
                        && saturation.is_finite()
                        && (0.0..=10.0).contains(saturation)
                }
                Effect::Crop {
                    x,
                    y,
                    width,
                    height,
                } => {
                    [x, y, width, height].iter().all(|v| v.is_finite())
                        && *x >= 0.0
                        && *y >= 0.0
                        && *width > 0.0
                        && *height > 0.0
                        && x + width <= 1.0
                        && y + height <= 1.0
                }
                Effect::Flip { .. } => true,
            };
            check!(
                valid,
                "invalid_effect",
                format!("{p}/effects/{j}"),
                "effect parameters are outside supported bounds"
            );
        }
    }
    let mut windows: Vec<(&str, i64, i64)> = Vec::new();
    for (i, transition) in doc.transitions.iter().enumerate() {
        let p = format!("/transitions/{i}");
        let (Some(from), Some(to)) = (
            doc.clip(&transition.from_clip),
            doc.clip(&transition.to_clip),
        ) else {
            check!(
                false,
                "unknown_clip",
                p,
                "transition references an unknown clip"
            );
            continue;
        };
        let cut = to.common().timeline_start;
        check!(
            from.common().track_id == to.common().track_id
                && from.end(fps) == cut
                && from.common().id != to.common().id,
            "invalid_transition",
            p,
            "transition clips must be adjacent on the same track"
        );
        let before = transition.duration / 2;
        let after = transition.duration - before;
        check!(
            transition.duration > 0 && before <= from.length(fps) && after <= to.length(fps),
            "invalid_transition",
            p,
            "transition window must fit both clips"
        );
        if transition.duration <= 0 || before > from.length(fps) || after > to.length(fps) {
            continue;
        }
        for (track, start, end) in &windows {
            if *track == from.common().track_id {
                check!(
                    cut.saturating_sub(before) >= *end || cut.saturating_add(after) <= *start,
                    "transition_overlap",
                    p,
                    "transition windows overlap"
                );
            }
        }
        windows.push((
            &from.common().track_id,
            cut.saturating_sub(before),
            cut.saturating_add(after),
        ));
        if let Clip::Media { source_in, .. } = to {
            check!(
                *source_in >= before,
                "insufficient_handles",
                p,
                "incoming source has insufficient pre-roll"
            );
        }
        if let Clip::Media {
            asset_id,
            source_out,
            ..
        } = from
            && let Some(info) = media.and_then(|m| m.get(asset_id))
        {
            check!(
                fps.seconds(source_out.saturating_add(after)) <= info.duration + 0.000001,
                "insufficient_handles",
                p,
                "outgoing source has insufficient post-roll"
            );
        }
        if let TransitionEffect::DipToColor { color: c } = &transition.effect {
            check!(
                color(c).is_some(),
                "invalid_color",
                format!("{p}/color"),
                "expected #RRGGBB or #RRGGBBAA"
            );
        }
    }
    findings
}

pub fn require_valid(doc: &Document, media: Option<&HashMap<String, MediaInfo>>) -> Result<()> {
    let findings = validate(doc, media);
    if let Some(finding) = findings.into_iter().next() {
        return Err(finding.error);
    }
    Ok(())
}
