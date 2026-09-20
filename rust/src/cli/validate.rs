use crate::engine::probe::MediaInfo;
use crate::timeline::{Clip, MediaKind, TimelineEditingState, TimelineTime, TrackKind};
use anyhow::{Error, Result, anyhow};
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
        state.serialize_field("message", &format!("{:?}", self.error))?;
        state.serialize_field("fix_hint", &self.fix_hint)?;
        state.end()
    }
}

pub fn validate(
    doc: &TimelineEditingState,
    media: Option<&HashMap<Ulid, MediaInfo>>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let settings = &doc.settings;
    let fps = settings.frame_rate;
    if !(2..=16384).contains(&settings.width) {
        findings.push(Finding {
            error: anyhow!(
                "invalid_dimensions: width must be 2..16384 (/settings/width) at {}:{}",
                file!(),
                line!()
            ),
            fix_hint: None,
        });
    }
    if !(2..=16384).contains(&settings.height) {
        findings.push(Finding {
            error: anyhow!(
                "invalid_dimensions: height must be 2..16384 (/settings/height) at {}:{}",
                file!(),
                line!()
            ),
            fix_hint: None,
        });
    }
    if !(fps.numerator > 0
        && fps.denominator > 0
        && fps.numerator <= i32::MAX as u32
        && fps.denominator <= i32::MAX as u32)
    {
        findings.push(Finding {
            error: anyhow!("invalid_frame_rate: frame rate components must be positive signed 32-bit integers (/settings/frame_rate) at {}:{}", file!(), line!()),
            fix_hint: None,
        });
    }
    if settings.audio_sample_rate < 8000 {
        findings.push(Finding {
            error: anyhow!("invalid_sample_rate: sample rate must be at least 8000 (/settings/audio_sample_rate) at {}:{}", file!(), line!()),
            fix_hint: None,
        });
    }
    let mut ids = HashSet::new();
    for (index, asset) in doc.assets.iter().enumerate() {
        if asset.id.is_nil() || !ids.insert(asset.id) {
            findings.push(Finding {
                error: anyhow!(
                    "duplicate_id: IDs must be nonzero and unique (/assets/{index}/id) at {}:{}",
                    file!(),
                    line!()
                ),
                fix_hint: None,
            });
        }
        if asset.path.as_os_str().is_empty() {
            findings.push(Finding {
                error: anyhow!(
                    "invalid_path: asset path cannot be empty (/assets/{index}/path) at {}:{}",
                    file!(),
                    line!()
                ),
                fix_hint: None,
            });
        }
        if !(asset.duration.is_finite() && asset.duration >= 0.0) {
            findings.push(Finding {
                error: anyhow!("invalid_duration: asset duration must be finite and nonnegative (/assets/{index}/duration) at {}:{}", file!(), line!()),
                fix_hint: None,
            });
        }
    }
    for (index, track) in doc.tracks.iter().enumerate() {
        if track.id.is_nil() || !ids.insert(track.id) {
            findings.push(Finding {
                error: anyhow!(
                    "duplicate_id: IDs must be nonzero and unique (/tracks/{index}/id) at {}:{}",
                    file!(),
                    line!()
                ),
                fix_hint: None,
            });
        }
    }
    for (index, clip) in doc.clips.iter().enumerate() {
        let pointer = format!("/clips/{index}/data");
        if clip.id().is_nil() || !ids.insert(clip.id()) {
            findings.push(Finding {
                error: anyhow!(
                    "duplicate_id: IDs must be nonzero and unique ({pointer}/id) at {}:{}",
                    file!(),
                    line!()
                ),
                fix_hint: None,
            });
        }
        if clip.timeline_start() < TimelineTime::ZERO {
            findings.push(Finding {
                error: anyhow!(
                    "invalid_time: start must be nonnegative ({pointer}/timeline_start) at {}:{}",
                    file!(),
                    line!()
                ),
                fix_hint: None,
            });
        }
        let length = clip.frame_length(fps);
        if !(length > TimelineTime::ZERO
            && clip
                .timeline_start()
                .frames()
                .checked_add(length.frames())
                .is_some())
        {
            findings.push(Finding {
                error: anyhow!("invalid_duration: clip duration must be positive and representable ({pointer}) at {}:{}", file!(), line!()),
                fix_hint: None,
            });
        }
        let Some(track) = doc.track(clip.track_id()) else {
            findings.push(Finding {
                error: anyhow!(
                    "unknown_track: clip references an unknown track ({pointer}/track_id) at {}:{}",
                    file!(),
                    line!()
                ),
                fix_hint: None,
            });
            continue;
        };
        if !(matches!(
            (track.kind, clip),
            (TrackKind::Video, Clip::Video(_))
                | (TrackKind::Audio, Clip::Audio(_))
                | (TrackKind::Text, Clip::Text(_))
        )) {
            findings.push(Finding {
                error: anyhow!(
                    "invalid_track: clip kind must match its track ({pointer}/track_id) at {}:{}",
                    file!(),
                    line!()
                ),
                fix_hint: None,
            });
        }
        match clip {
            Clip::Video(data) | Clip::Audio(data) => {
                if !(data.source_in >= TimelineTime::ZERO && data.source_out > data.source_in) {
                    findings.push(Finding {
                        error: anyhow!("invalid_trim: source range must be nonnegative and nonempty ({pointer}) at {}:{}", file!(), line!()),
                        fix_hint: None,
                    });
                }
                let video = data.video_properties;
                if !(video.position_x.is_finite()
                    && video.position_y.is_finite()
                    && video.scale.is_finite()
                    && video.scale >= 0.0)
                {
                    findings.push(Finding {
                        error: anyhow!("invalid_property: video transform must be finite with nonnegative scale ({pointer}/video_properties) at {}:{}", file!(), line!()),
                        fix_hint: None,
                    });
                }
                if !(data.audio_properties.gain_db.is_finite()) {
                    findings.push(Finding {
                        error: anyhow!("invalid_property: gain must be finite ({pointer}/audio_properties/gain_db) at {}:{}", file!(), line!()),
                        fix_hint: None,
                    });
                }
                let Some(asset) = doc.asset(data.asset_id) else {
                    findings.push(Finding {
                            error: anyhow!("unknown_asset: clip references an unknown asset ({pointer}/asset_id) at {}:{}", file!(), line!()),
                            fix_hint: None,
                        });
                    continue;
                };
                if !(track.kind != TrackKind::Video || asset.kind != MediaKind::Audio) {
                    findings.push(Finding {
                        error: anyhow!("invalid_track: audio assets cannot be placed on a video track ({pointer}) at {}:{}", file!(), line!()),
                        fix_hint: None,
                    });
                }
                let Some(media) = media else {
                    continue;
                };
                let Some(info) = media.get(&data.asset_id) else {
                    continue;
                };
                if !(asset.kind == MediaKind::Image || data.source_out <= fps.ceil(info.duration)) {
                    findings.push(Finding {
                        error: anyhow!("source_bounds: source range exceeds the media duration ({pointer}/source_out) at {}:{}", file!(), line!()),
                        fix_hint: None,
                    });
                }
                if !(track.kind != TrackKind::Video || info.video || info.image) {
                    findings.push(Finding {
                        error: anyhow!(
                            "missing_video: video clip requires visual media ({pointer}) at {}:{}",
                            file!(),
                            line!()
                        ),
                        fix_hint: None,
                    });
                }
                if !(track.kind != TrackKind::Audio || info.audio) {
                    findings.push(Finding {
                        error: anyhow!("missing_audio: audio clip requires an audio stream ({pointer}) at {}:{}", file!(), line!()),
                        fix_hint: None,
                    });
                }
            }
            Clip::Text(text) => {
                let properties = &text.properties;
                if !(properties.font_size.is_finite()
                    && properties.font_size > 0.0
                    && properties.position_x.is_finite()
                    && properties.position_y.is_finite())
                {
                    findings.push(Finding {
                        error: anyhow!("invalid_property: text size and position must be finite with positive size ({pointer}/properties) at {}:{}", file!(), line!()),
                        fix_hint: None,
                    });
                }
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
            if clip.timeline_start() < end {
                findings.push(Finding {
                    error: anyhow!("overlap: clips on the same track cannot overlap (/clips/{index}/data/timeline_start) at {}:{}", file!(), line!()),
                    fix_hint: None,
                });
            }
            end = end.max(clip.timeline_end(fps));
        }
    }
    findings
}

pub fn require_valid(
    doc: &TimelineEditingState,
    media: Option<&HashMap<Ulid, MediaInfo>>,
) -> Result<()> {
    let Some(finding) = validate(doc, media).into_iter().next() else {
        return Ok(());
    };
    Err(finding.error)
}
