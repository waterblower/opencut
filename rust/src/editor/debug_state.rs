use super::{Editor, preview::PreviewTarget};
use anyhow::{Context as _, Result};
use gpui::{ClipboardItem, Context};
use gstreamer as gst;
use gstreamer_editing_services::prelude::*;
use gstreamer_video::{VideoFrameExt, VideoFrameRef, VideoInfo};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

impl Editor {
    pub fn copy_debug_state(&mut self, cx: &mut Context<Self>) {
        match debug_state(self) {
            Ok(report) => {
                cx.write_to_clipboard(ClipboardItem::new_string(report));
                self.status = Some("Debug state copied. Paste it into the bug report.".into());
            }
            Err(error) => {
                log::error!("Could not dump debug state: {error:?}");
                self.status = Some(format!("Could not dump debug state: {error:?}"));
            }
        }
        cx.notify();
    }
}

fn debug_state(editor: &Editor) -> Result<String> {
    let target = match &editor.preview.target {
        PreviewTarget::None => "None",
        PreviewTarget::Timeline => "Timeline",
        PreviewTarget::VideoFile(_, _) => "VideoFile",
        PreviewTarget::AudioFile(_, _) => "AudioFile",
        PreviewTarget::ImageFile(_) => "ImageFile",
    };
    // Deliberately select fields rather than serializing the application/global settings.
    let mut report = json!({
        "format": "OpenCut debug state v1",
        "version": env!("CARGO_PKG_VERSION"),
        "captured_unix_ms": SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis(),
        "notes": "Live snapshot; fields may change during playback. No credentials, environment, or raw media bytes are included. Timeline text and asset paths are included.",
        "preview": {
            "target": target,
            "fullscreen": editor.preview.fullscreen,
            "scrubbing": editor.preview.is_scrubbing,
            "transform_drag_active": editor.preview.timeline_drag.is_some(),
        },
        "status": editor.status,
        "export_running": editor.export.running,
        "timeline": null,
    });
    if let Some(timeline) = &editor.timeline {
        let playback = timeline.video_backend.playback();
        let pipeline = playback.pipeline();
        let position_before = playback.position();
        let (state_result, state, pending) = pipeline.state(gst::ClockTime::ZERO);
        let mut layers = Vec::new();
        for layer in timeline.video_backend.ges_timeline().layers() {
            let mut clips = Vec::new();
            for clip in layer.clips() {
                let mut properties = serde_json::Map::new();
                for name in [
                    "text",
                    "font-desc",
                    "color",
                    "foreground-color",
                    "background-color",
                    "alpha",
                    "posx",
                    "posy",
                    "xpos",
                    "ypos",
                    "width",
                    "height",
                    "text-x",
                    "text-y",
                    "text-width",
                    "text-height",
                ] {
                    let Some((object, property)) = clip.lookup_child(name) else {
                        continue;
                    };
                    if property.flags().contains(gst::glib::ParamFlags::READABLE) {
                        properties.insert(name.into(), property_snapshot(&object, property.name()));
                    }
                }
                clips.push(json!({
                    "name": clip.name().as_deref(), "type": clip.type_().name(),
                    "start_ns": clip.start().nseconds(), "duration_ns": clip.duration().nseconds(),
                    "inpoint_ns": clip.inpoint().nseconds(),
                    "formats": format!("{:?}", clip.supported_formats()),
                    "properties": properties,
                }));
            }
            layers.push(json!({"priority": layer.priority(), "clips": clips}));
        }
        let mut elements = Vec::new();
        let mut iterator = pipeline.iterate_recurse();
        let mut graph_note = None;
        loop {
            if elements.len() == 256 {
                graph_note = Some("Element list capped at 256".to_string());
                break;
            }
            let element = match iterator.next() {
                Ok(Some(element)) => element,
                Ok(None) => break,
                Err(error) => {
                    graph_note = Some(format!(
                        "Graph changed or could not be read: {error:?} at {}:{}",
                        file!(),
                        line!()
                    ));
                    break;
                }
            };
            let (_, state, pending) = element.state(gst::ClockTime::ZERO);
            let mut pads = Vec::new();
            for pad in element.pads() {
                let caps = pad.current_caps();
                pads.push(json!({
                    "name": pad.name().as_str(), "direction": format!("{:?}", pad.direction()),
                    "linked": pad.is_linked(), "active": pad.is_active(),
                    "caps": caps.as_ref().map(|caps| caps.to_string()),
                    "properties": object_properties(pad.upcast_ref()),
                }));
            }
            elements.push(json!({
                "name": element.name().as_str(),
                "factory": element.factory().as_ref().map(|factory| factory.name().to_string()),
                "state": format!("{state:?}"), "pending": format!("{pending:?}"),
                "properties": object_properties(element.upcast_ref()), "pads": pads,
            }));
        }
        let frame = match playback.get_current_frame() {
            Some(sample) => frame_snapshot(&sample),
            None => json!({"present": false}),
        };
        report["timeline"] = json!({
            "path": timeline.path,
            "in_memory": serde_json::to_value(&timeline.data).context(format!("Serializing live timeline at {}:{}", file!(), line!()))?,
            "selected_clip_id": timeline.interaction.selected_clip_id,
            "selected_clip_ids": timeline.interaction.selected_clip_ids,
            "undo_count": timeline.undo_stack.len(), "redo_count": timeline.redo_stack.len(),
            "playback": {
                "position_before_ns": position_before.as_nanos(),
                "position_after_ns": playback.position().as_nanos(),
                "duration_ns": playback.duration().as_nanos(),
                "state": format!("{state:?}"), "pending": format!("{pending:?}"),
                "state_result": format!("{state_result:?}"),
                "volume": playback.volume(), "frame": frame,
            },
            "ges_layers": layers, "pipeline_elements": elements, "graph_note": graph_note,
        });
    }
    let json = serde_json::to_string_pretty(&report).context(format!(
        "Formatting debug state at {}:{}",
        file!(),
        line!()
    ))?;
    Ok(format!("OpenCut debug state\n```json\n{json}\n```"))
}

fn object_properties(object: &gst::glib::Object) -> Value {
    let mut properties = serde_json::Map::new();
    // Never dump arbitrary object properties: URI/location and other strings may contain secrets.
    for name in [
        "alpha",
        "posx",
        "posy",
        "xpos",
        "ypos",
        "width",
        "height",
        "zorder",
        "operator",
        "foreground-color",
        "background-color",
        "pattern",
        "silent",
        "sync",
        "qos",
        "is-live",
        "ignore-inactive-pads",
        "repeat-after-eos",
        "max-last-buffer-repeat",
        "drop",
        "max-buffers",
    ] {
        let Some(property) = object.find_property(name) else {
            continue;
        };
        if property.flags().contains(gst::glib::ParamFlags::READABLE) {
            properties.insert(name.into(), property_snapshot(object, name));
        }
    }
    Value::Object(properties)
}

fn property_snapshot(object: &gst::glib::Object, name: &str) -> Value {
    match object.property_value(name).serialize() {
        Ok(value) => json!(value.as_str()),
        Err(error) => json!({"error": format!("{error:?} at {}:{}", file!(), line!())}),
    }
}

fn frame_snapshot(sample: &gst::Sample) -> Value {
    let mut result = json!({"present": true, "caps": sample.caps().map(|caps| caps.to_string())});
    let Some(buffer) = sample.buffer() else {
        return result;
    };
    result["pts_ns"] = json!(buffer.pts().map(|time| time.nseconds()));
    result["dts_ns"] = json!(buffer.dts().map(|time| time.nseconds()));
    result["duration_ns"] = json!(buffer.duration().map(|time| time.nseconds()));
    result["bytes"] = json!(buffer.size());
    result["flags"] = json!(format!("{:?}", buffer.flags()));
    let Some(caps) = sample.caps() else {
        return result;
    };
    let info = match VideoInfo::from_caps(caps) {
        Ok(info) => info,
        Err(error) => {
            result["error"] = json!(format!("Video caps: {error:?} at {}:{}", file!(), line!()));
            return result;
        }
    };
    result["width"] = json!(info.width());
    result["height"] = json!(info.height());
    if info.format() != gstreamer_video::VideoFormat::Nv12 {
        return result;
    }
    let frame = match VideoFrameRef::from_buffer_ref_readable(buffer, &info) {
        Ok(frame) => frame,
        Err(error) => {
            result["error"] = json!(format!(
                "Reading frame: {error:?} at {}:{}",
                file!(),
                line!()
            ));
            return result;
        }
    };
    result["strides"] = json!(frame.info().stride());
    let plane = match frame.plane_data(0) {
        Ok(plane) => plane,
        Err(error) => {
            result["error"] = json!(format!(
                "Reading luma: {error:?} at {}:{}",
                file!(),
                line!()
            ));
            return result;
        }
    };
    let Ok(stride) = usize::try_from(frame.info().stride()[0]) else {
        return result;
    };
    let mut histogram = [0_u64; 256];
    for row in 0..frame.height() as usize {
        let start = row * stride;
        let Some(pixels) = plane.get(start..start + frame.width() as usize) else {
            return result;
        };
        for &pixel in pixels {
            histogram[usize::from(pixel)] += 1;
        }
    }
    let count: u64 = histogram.iter().sum();
    let mut sum = 0_u64;
    for (value, count) in histogram.iter().enumerate() {
        sum += value as u64 * count;
    }
    result["luma"] = json!({
        "sample_count": count,
        "mean": if count == 0 { 0.0 } else { sum as f64 / count as f64 },
        "histogram": histogram.as_slice(),
    });
    result
}

#[cfg(test)]
#[path = "tests/debug_state.test.rs"]
mod tests;
