use super::opacity_from_pointer;

#[test]
fn opacity_slider_maps_and_clamps_pointer_positions() {
    assert_eq!(opacity_from_pointer(50.0, 100.0, 200.0), 0.0);
    assert_eq!(opacity_from_pointer(200.0, 100.0, 200.0), 0.5);
    assert_eq!(opacity_from_pointer(350.0, 100.0, 200.0), 1.0);
}
