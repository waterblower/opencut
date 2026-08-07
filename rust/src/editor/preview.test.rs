use super::*;

#[test]
fn visual_layout_maps_project_position_and_scale_to_preview_pixels() {
    let plan = resolve_visual_clip_render_plan(
        VideoClipProperties {
            position_x: 100.0,
            position_y: -50.0,
            scale: 0.5,
            ..VideoClipProperties::default()
        },
        1920,
        1080,
        1920,
        1080,
        960.0,
        540.0,
    );

    assert_eq!(plan.uncropped.left, 290.0);
    assert_eq!(plan.uncropped.top, 110.0);
    assert_eq!(plan.uncropped.width, 480.0);
    assert_eq!(plan.uncropped.height, 270.0);
}

#[test]
fn video_rasterization_is_reserved_for_opacity_and_crop() {
    let plan_for = |properties| {
        resolve_visual_clip_render_plan(properties, 1920, 1080, 1920, 1080, 960.0, 540.0)
    };
    assert!(
        !plan_for(VideoClipProperties {
            position_x: 20.0,
            position_y: -20.0,
            scale: 1.5,
            ..VideoClipProperties::default()
        })
        .requires_rasterization()
    );
    assert!(
        plan_for(VideoClipProperties {
            opacity: 0.5,
            ..VideoClipProperties::default()
        })
        .requires_rasterization()
    );
    assert!(
        plan_for(VideoClipProperties {
            crop_left: 0.1,
            ..VideoClipProperties::default()
        })
        .requires_rasterization()
    );
}
