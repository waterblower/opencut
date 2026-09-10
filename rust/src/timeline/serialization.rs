use super::{Clip, FrameRate, TimelineSerialization, TimelineTime, TrackKind};
use serde::Serialize;
use serde_json::Value;
use std::fmt;

#[derive(Debug, Serialize)]
pub struct ParseError {
    pub code: &'static str,
    pub pointer: String,
    pub message: String,
    pub file: &'static str,
    pub line: u32,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} ({}, {}:{})",
            self.code, self.message, self.pointer, self.file, self.line
        )
    }
}
impl std::error::Error for ParseError {}

/// Deserialize the GUI timeline representation, including its existing aliases.
/// The input and output are in memory; callers own file I/O and validation policy.
pub fn parse(value: &Value) -> Result<TimelineSerialization, ParseError> {
    if value.get("version").is_some()
        || value.get("transitions").is_some()
        || value
            .get("clips")
            .and_then(Value::as_array)
            .is_some_and(|clips| clips.iter().any(|clip| clip.get("type").is_some()))
    {
        return Err(ParseError {
            code: "legacy_cli_format",
            pointer: String::new(),
            message:
                "the legacy CLI timeline is unsupported; use the shared editor timeline format"
                    .into(),
            file: file!(),
            line: line!(),
        });
    }
    let mut value = value.clone();
    let frame_rate = match value.pointer("/settings/frame_rate") {
        Some(rate) => match serde_json::from_value::<FrameRate>(rate.clone()) {
            Ok(rate) => rate,
            Err(error) => {
                return Err(ParseError {
                    code: "schema_error",
                    pointer: "/settings/frame_rate".into(),
                    message: error.to_string(),
                    file: file!(),
                    line: line!(),
                });
            }
        },
        None => FrameRate::default(),
    };
    if let Some(clips) = value.get_mut("clips").and_then(Value::as_array_mut) {
        for clip in clips {
            let Some(clip) = clip.as_object_mut() else {
                continue;
            };
            if !clip.contains_key("text") && !clip.contains_key("properties") {
                continue;
            }
            let Some(frames) = clip.get("length").and_then(Value::as_i64) else {
                continue;
            };
            let duration = frame_rate.duration(TimelineTime::from_frames(frames));
            clip.insert(
                "length".into(),
                serde_json::json!({"secs": duration.as_secs(), "nanos": duration.subsec_nanos()}),
            );
        }
    }
    match serde_path_to_error::deserialize::<_, TimelineSerialization>(&value) {
        Ok(mut document) => {
            for clip in &mut document.clips {
                let track_kind = document
                    .tracks
                    .iter()
                    .find(|track| track.id == clip.track_id())
                    .map(|track| track.kind);
                let replacement = match (track_kind, &*clip) {
                    (Some(TrackKind::Audio), Clip::Video(data)) => Some(Clip::Audio(data.clone())),
                    (Some(TrackKind::Video), Clip::Audio(data)) => Some(Clip::Video(data.clone())),
                    _ => None,
                };
                if let Some(replacement) = replacement {
                    *clip = replacement;
                }
            }
            Ok(document)
        }
        Err(error) => {
            let mut pointer = String::new();
            for segment in error.path() {
                let token = match segment {
                    serde_path_to_error::Segment::Seq { index } => index.to_string(),
                    serde_path_to_error::Segment::Map { key } => key.clone(),
                    serde_path_to_error::Segment::Enum { variant } => variant.clone(),
                    serde_path_to_error::Segment::Unknown => continue,
                };
                pointer.push('/');
                pointer.push_str(&token.replace('~', "~0").replace('/', "~1"));
            }
            Err(ParseError {
                code: "schema_error",
                pointer,
                message: error.inner().to_string(),
                file: file!(),
                line: line!(),
            })
        }
    }
}
