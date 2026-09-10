use crate::args::{Edit, Kind};
use opencut_player::{
    cli_error, cli_try,
    core::{
        document::{self, Clip},
        error::Result,
        validate::require_valid,
    },
    engine::probe,
};
use serde_json::{Value, json};
use std::path::Path;
use ulid::Ulid;

pub fn edit(path: &Path, command: Edit) -> Result<Value> {
    let (mut raw, doc) = document::load(path)?;
    require_valid(&doc, None)?;
    let base = path.parent().unwrap_or(Path::new("."));
    let fps = doc.settings.frame_rate;
    let result = match command {
        Edit::AddTrack { kind, name } => {
            let kind = match kind {
                Kind::Video => "video",
                Kind::Audio => "audio",
                Kind::Text => "text",
            };
            let track = json!({"id": Ulid::generate().to_string(), "kind": kind, "name": name.unwrap_or(kind.into()), "muted": false});
            raw["tracks"].as_array_mut().unwrap().push(track.clone());
            track
        }
        Edit::AddClip {
            track,
            asset,
            at,
            source_in,
            source_out,
        } => {
            let full_path = cli_try!(std::fs::canonicalize(&asset), "unreadable_media", "", 4);
            let info = probe::probe(&full_path)?;
            let mut asset_id = None;
            for a in &doc.assets {
                if let Ok(existing) = std::fs::canonicalize(base.join(&a.path))
                    && existing == full_path
                {
                    asset_id = Some(a.id.clone());
                    break;
                }
            }
            let asset_id = match asset_id {
                Some(id) => id,
                None => {
                    let id = Ulid::generate().to_string();
                    let base_path = cli_try!(std::fs::canonicalize(base), "io_error", "", 6);
                    let stored = full_path.strip_prefix(&base_path).unwrap_or(&full_path);
                    raw["assets"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"id": id, "path": stored}));
                    id
                }
            };
            let start = fps.parse_time(&at, None)?;
            let input = fps.parse_time(&source_in, None)?;
            let clip = if probe::is_image(&full_path) {
                if input != 0 {
                    return Err(cli_error!(
                        "invalid_trim",
                        "",
                        2,
                        "images do not have source trims"
                    ));
                }
                let length = match source_out {
                    Some(out) => fps.parse_time(&out, None)?,
                    None => fps.parse_time("5s", None)?,
                };
                json!({"type": "image", "id": Ulid::generate().to_string(), "track_id": track, "asset_id": asset_id, "timeline_start": start, "length": length})
            } else {
                let out = match source_out {
                    Some(out) => fps.parse_time(&out, None)?,
                    None => (info.duration * fps.numerator as f64 / fps.denominator as f64).floor()
                        as i64,
                };
                json!({"type": "media", "id": Ulid::generate().to_string(), "track_id": track, "asset_id": asset_id, "timeline_start": start, "source_in": input, "source_out": out})
            };
            raw["clips"].as_array_mut().unwrap().push(clip.clone());
            clip
        }
        Edit::AddText {
            track,
            text,
            at,
            duration,
            font,
            size,
            color,
            pos,
        } => {
            let Some((x, y)) = pos.split_once(',') else {
                return Err(cli_error!("invalid_position", "", 2, "expected x,y"));
            };
            let x: f64 = cli_try!(x.parse(), "invalid_position", "", 2);
            let y: f64 = cli_try!(y.parse(), "invalid_position", "", 2);
            let Some(rgba) = document::color(&color) else {
                return Err(cli_error!(
                    "invalid_color",
                    "",
                    2,
                    "expected #RRGGBB or #RRGGBBAA"
                ));
            };
            let clip = json!({"type": "text", "id": Ulid::generate().to_string(), "track_id": track, "timeline_start": fps.parse_time(&at, None)?, "length": fps.parse_time(&duration, None)?, "properties": {"text": text, "font": font, "font_size": size, "color": u32::from_be_bytes(rgba), "position_x": x, "position_y": y}});
            raw["clips"].as_array_mut().unwrap().push(clip.clone());
            clip
        }
        Edit::MoveClip { clip, to, track } => {
            let i = index(&doc, &clip)?;
            raw["clips"][i]["timeline_start"] = json!(fps.parse_time(&to, None)?);
            if let Some(track) = track {
                raw["clips"][i]["track_id"] = json!(track);
            }
            raw["clips"][i].clone()
        }
        Edit::TrimClip {
            clip,
            source_in,
            source_out,
        } => {
            let i = index(&doc, &clip)?;
            if !matches!(doc.clips[i], Clip::Media { .. }) {
                return Err(cli_error!(
                    "invalid_trim",
                    "",
                    2,
                    "trim-clip requires a media clip"
                ));
            }
            if source_in.is_none() && source_out.is_none() {
                return Err(cli_error!("invalid_trim", "", 2, "provide --in or --out"));
            }
            if let Some(value) = source_in {
                raw["clips"][i]["source_in"] = json!(fps.parse_time(&value, None)?);
            }
            if let Some(value) = source_out {
                raw["clips"][i]["source_out"] = json!(fps.parse_time(&value, None)?);
            }
            raw["clips"][i].clone()
        }
        Edit::SplitClip { clip, at } => {
            let i = index(&doc, &clip)?;
            let original = &doc.clips[i];
            let at = fps.parse_time(&at, None)?;
            let offset = at - original.common().timeline_start;
            if offset <= 0 || at >= original.end(fps) {
                return Err(cli_error!(
                    "invalid_split",
                    "",
                    3,
                    "split must be strictly inside the clip"
                ));
            }
            let mut right = raw["clips"][i].clone();
            let right_id = Ulid::generate().to_string();
            right["id"] = json!(right_id);
            right["timeline_start"] = json!(at);
            match original {
                Clip::Media { source_in, .. } => {
                    raw["clips"][i]["source_out"] = json!(source_in + offset);
                    right["source_in"] = json!(source_in + offset);
                }
                _ => {
                    set_length(&mut raw["clips"][i]["length"], offset, fps);
                    set_length(&mut right["length"], original.length(fps) - offset, fps);
                }
            }
            if let Some(Value::Array(transitions)) = raw.get_mut("transitions") {
                for transition in transitions {
                    if transition["from_clip"] == clip {
                        transition["from_clip"] = json!(right_id);
                    }
                }
            }
            let left = raw["clips"][i].clone();
            raw["clips"]
                .as_array_mut()
                .unwrap()
                .insert(i + 1, right.clone());
            json!({"clips": [left, right]})
        }
        Edit::RemoveClip { clip } => {
            let i = index(&doc, &clip)?;
            let removed = raw["clips"].as_array_mut().unwrap().remove(i);
            if let Some(Value::Array(transitions)) = raw.get_mut("transitions") {
                transitions.retain(|t| t["from_clip"] != clip && t["to_clip"] != clip);
            }
            removed
        }
        Edit::Set {
            clip,
            property,
            value,
        } => {
            let i = index(&doc, &clip)?;
            let parts: Vec<_> = property.split('.').collect();
            let allowed = match parts.as_slice() {
                ["opacity" | "effects"] => true,
                ["video_properties", "position_x" | "position_y" | "scale"] => {
                    !matches!(doc.clips[i], Clip::Text { .. })
                }
                ["audio_properties", "gain_db" | "muted"] => {
                    matches!(doc.clips[i], Clip::Media { .. })
                }
                [
                    "properties",
                    "text" | "font" | "font_size" | "color" | "position_x" | "position_y",
                ] => matches!(doc.clips[i], Clip::Text { .. }),
                _ => false,
            };
            if !allowed {
                return Err(cli_error!(
                    "invalid_property",
                    "",
                    2,
                    "property is not editable: {property}"
                ));
            }
            let parsed = match serde_json::from_str::<Value>(&value) {
                Ok(v) => v,
                Err(_) => Value::String(value),
            };
            if parts.len() == 1 {
                raw["clips"][i][parts[0]] = parsed;
            } else {
                if raw["clips"][i].get(parts[0]).is_none() {
                    raw["clips"][i][parts[0]] = json!({});
                }
                raw["clips"][i][parts[0]][parts[1]] = parsed;
            }
            raw["clips"][i].clone()
        }
    };
    let updated = document::parse(&raw)?;
    require_valid(&updated, None)?;
    let media = probe::assets(&updated, base)?;
    require_valid(&updated, Some(&media))?;
    document::write_atomic(path, &raw, true)?;
    Ok(result)
}

fn index(doc: &document::Document, id: &str) -> Result<usize> {
    let Some(index) = doc.clips.iter().position(|c| c.common().id == id) else {
        return Err(cli_error!(
            "unknown_clip",
            "/clips",
            3,
            "clip {id} does not exist"
        ));
    };
    Ok(index)
}

fn set_length(value: &mut Value, frames: i64, fps: opencut_player::core::time::FrameRate) {
    if value.is_object() {
        let n = frames as u128 * fps.denominator as u128 * 1_000_000_000;
        let nanos = (n + fps.numerator as u128 / 2) / fps.numerator as u128;
        value["secs"] = json!((nanos / 1_000_000_000) as u64);
        value["nanos"] = json!((nanos % 1_000_000_000) as u32);
    } else {
        *value = json!(frames);
    }
}
