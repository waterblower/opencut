use super::*;

#[test]
fn completed_open_only_installs_into_the_requested_file_preview() {
    let root = Path::new("/project");
    let file = Path::new("video.mp4");
    assert!(file_preview_requested(
        root,
        Some(file),
        &PreviewTarget::None,
        root,
        file
    ));
    assert!(!file_preview_requested(
        root,
        Some(Path::new("other.mp4")),
        &PreviewTarget::None,
        root,
        file
    ));
    assert!(!file_preview_requested(
        Path::new("/other-project"),
        Some(file),
        &PreviewTarget::None,
        root,
        file
    ));
    assert!(!file_preview_requested(
        root,
        Some(file),
        &PreviewTarget::Timeline,
        root,
        file
    ));
    assert!(!file_preview_requested(
        root,
        Some(file),
        &PreviewTarget::ImageFile("image.png".into()),
        root,
        file
    ));
    assert!(!file_preview_requested(
        root,
        None,
        &PreviewTarget::None,
        root,
        file
    ));
}
