use super::*;

#[test]
fn opacity_is_applied_to_transformed_pixels() {
    let source = RgbaImage::from_pixel(1, 1, Rgba([10, 20, 30, 200]));
    let output = transform_frame(
        &source,
        1,
        1,
        1,
        1,
        VideoClipProperties {
            opacity: 0.5,
            ..VideoClipProperties::default()
        },
    );
    assert_eq!(output.get_pixel(0, 0), &Rgba([10, 20, 30, 100]));
}

#[test]
fn position_uses_project_pixels() {
    let source = RgbaImage::from_pixel(1, 1, Rgba([255, 0, 0, 255]));
    let output = transform_frame(
        &source,
        4,
        2,
        4,
        2,
        VideoClipProperties {
            position_x: 1.0,
            ..VideoClipProperties::default()
        },
    );
    assert_eq!(output.get_pixel(3, 1), &Rgba([255, 0, 0, 255]));
    assert_eq!(output.get_pixel(1, 1), &Rgba([0, 0, 0, 0]));
}

#[test]
fn crop_removes_pixels_outside_the_visible_source_area() {
    let source = RgbaImage::from_pixel(4, 1, Rgba([255, 0, 0, 255]));
    let output = transform_frame(
        &source,
        4,
        1,
        4,
        1,
        VideoClipProperties {
            crop_left: 0.25,
            crop_right: 0.25,
            ..VideoClipProperties::default()
        },
    );

    assert_eq!(output.get_pixel(0, 0), &Rgba([0, 0, 0, 0]));
    assert_eq!(output.get_pixel(1, 0), &Rgba([255, 0, 0, 255]));
    assert_eq!(output.get_pixel(2, 0), &Rgba([255, 0, 0, 255]));
    assert_eq!(output.get_pixel(3, 0), &Rgba([0, 0, 0, 0]));
}
