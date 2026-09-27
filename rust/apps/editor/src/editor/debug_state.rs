use crate::editor::{Editor, preview::PreviewTarget};
use anyhow::{Context as _, Result};
use gpui::{ClipboardItem, Context};
use serde_json::json;
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
        },
        "status": editor.status,
        "timeline": null,
    });
    if let Some(timeline) = &editor.timeline {
        report["timeline"] = json!({
            "path": timeline.path,
            "in_memory": serde_json::to_value(timeline.to_serialize()).context("Serializing live timeline")?,
            "selected_clip_id": timeline.interaction.selected_clip_id,
            "selected_clip_ids": timeline.interaction.selected_clip_ids,
            "undo_count": timeline.undo_stack.len(), "redo_count": timeline.redo_stack.len(),
            "playhead_frame": timeline.playhead().frames(),
        });
    }
    let json = serde_json::to_string_pretty(&report).context("Formatting debug state")?;
    Ok(format!("OpenCut debug state\n```json\n{json}\n```"))
}
