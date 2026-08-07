use super::model::{AudioClipProperties, VideoClipProperties};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct RenderRect {
    pub(super) left: f64,
    pub(super) top: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct SourceCrop {
    pub(super) left: u32,
    pub(super) right: u32,
    pub(super) top: u32,
    pub(super) bottom: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct VisualClipRenderPlan {
    pub(super) properties: VideoClipProperties,
    pub(super) uncropped: RenderRect,
    pub(super) visible: RenderRect,
    pub(super) crop: SourceCrop,
    pub(super) source_scale: f64,
    pub(super) opacity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AudioClipRenderPlan {
    pub(super) gain_linear: f64,
    pub(super) muted: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_visual_clip_render_plan(
    properties: VideoClipProperties,
    source_width: u32,
    source_height: u32,
    project_width: u32,
    project_height: u32,
    target_width: f64,
    target_height: f64,
) -> VisualClipRenderPlan {
    let properties = sanitize_video_properties(properties);
    let source_width = source_width.max(1);
    let source_height = source_height.max(1);
    let project_width = project_width.max(1) as f64;
    let project_height = project_height.max(1) as f64;
    let target_width = finite_or(target_width, 1.0).max(1.0);
    let target_height = finite_or(target_height, 1.0).max(1.0);
    let project_scale = (target_width / project_width).min(target_height / project_height);
    let fitted_scale = (project_width * project_scale / source_width as f64)
        .min(project_height * project_scale / source_height as f64);
    let source_scale = fitted_scale * properties.scale;

    let crop_left = crop_pixels(source_width, properties.crop_left, 0);
    let crop_right = crop_pixels(source_width, properties.crop_right, crop_left);
    let crop_top = crop_pixels(source_height, properties.crop_top, 0);
    let crop_bottom = crop_pixels(source_height, properties.crop_bottom, crop_top);
    let crop = SourceCrop {
        left: crop_left,
        right: crop_right,
        top: crop_top,
        bottom: crop_bottom,
    };

    let full_width = source_width as f64 * source_scale;
    let full_height = source_height as f64 * source_scale;
    let center_x = target_width * 0.5 + properties.position_x * project_scale;
    let center_y = target_height * 0.5 + properties.position_y * project_scale;
    let uncropped = RenderRect {
        left: center_x - full_width * 0.5,
        top: center_y - full_height * 0.5,
        width: full_width,
        height: full_height,
    };
    let visible_source_width = source_width.saturating_sub(crop_left + crop_right).max(1);
    let visible_source_height = source_height.saturating_sub(crop_top + crop_bottom).max(1);
    let visible = RenderRect {
        left: uncropped.left + crop_left as f64 * source_scale,
        top: uncropped.top + crop_top as f64 * source_scale,
        width: visible_source_width as f64 * source_scale,
        height: visible_source_height as f64 * source_scale,
    };

    VisualClipRenderPlan {
        properties,
        uncropped,
        visible,
        crop,
        source_scale,
        opacity: if source_scale <= f64::EPSILON {
            0.0
        } else {
            properties.opacity
        },
    }
}

pub(super) fn resolve_audio_clip_render_plan(
    track_muted: bool,
    properties: AudioClipProperties,
) -> AudioClipRenderPlan {
    let gain_db = finite_or(properties.gain_db, 0.0).clamp(-96.0, 24.0);
    AudioClipRenderPlan {
        gain_linear: 10.0f64.powf(gain_db / 20.0),
        muted: track_muted || properties.muted,
    }
}

fn sanitize_video_properties(mut properties: VideoClipProperties) -> VideoClipProperties {
    properties.position_x = finite_or(properties.position_x, 0.0);
    properties.position_y = finite_or(properties.position_y, 0.0);
    properties.scale = finite_or(properties.scale, 1.0).max(0.0);
    properties.opacity = finite_or(properties.opacity, 1.0).clamp(0.0, 1.0);
    properties.crop_left = finite_or(properties.crop_left, 0.0).clamp(0.0, 0.99);
    properties.crop_right =
        finite_or(properties.crop_right, 0.0).clamp(0.0, 0.99 - properties.crop_left);
    properties.crop_top = finite_or(properties.crop_top, 0.0).clamp(0.0, 0.99);
    properties.crop_bottom =
        finite_or(properties.crop_bottom, 0.0).clamp(0.0, 0.99 - properties.crop_top);
    properties
}

fn crop_pixels(size: u32, fraction: f64, already_cropped: u32) -> u32 {
    ((size as f64 * fraction).round() as u32).min(size.saturating_sub(already_cropped + 1))
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
#[path = "clip_render_plan.test.rs"]
mod tests;
