use super::*;

#[test]
fn resolves_visual_geometry_in_target_pixels() {
    let plan = resolve_visual_clip_render_plan(
        VideoClipProperties {
            position_x: 120.0,
            position_y: -60.0,
            scale: 0.5,
            opacity: 0.25,
        },
        320,
        180,
        1920,
        1080,
        1920.0,
        1080.0,
    );

    assert_eq!(plan.visible.left, 600.0);
    assert_eq!(plan.visible.top, 210.0);
    assert_eq!(plan.visible.width, 960.0);
    assert_eq!(plan.visible.height, 540.0);
    assert_eq!(plan.opacity, 0.25);
}

#[test]
fn resolves_audio_gain_and_mute() {
    let plan = resolve_audio_clip_render_plan(
        false,
        AudioClipProperties {
            gain_db: 6.0,
            muted: false,
        },
    );
    assert!((plan.gain_linear - 1.995_262_314_968_879_5).abs() < 0.000_001);
    assert!(!plan.muted);
    assert!(resolve_audio_clip_render_plan(true, AudioClipProperties::default()).muted);
}
