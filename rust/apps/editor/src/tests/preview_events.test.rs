use super::*;
use crate::preview::PreviewTarget;

#[test]
fn completed_open_only_installs_into_the_requested_file_preview() {
    let root = Path::new("/project");
    let file = Path::new("video.mp4");
    for (project, selected, target, expected) in [
        (root, Some(file), PreviewTarget::None, true),
        (
            root,
            Some(Path::new("other.mp4")),
            PreviewTarget::None,
            false,
        ),
        (
            Path::new("/other-project"),
            Some(file),
            PreviewTarget::None,
            false,
        ),
        (
            root,
            Some(file),
            PreviewTarget::ImageFile("image.png".into()),
            false,
        ),
        (root, None, PreviewTarget::None, false),
    ] {
        assert_eq!(
            file_preview_requested(project, selected, &target, root, file),
            expected,
            "project={project:?}, selected={selected:?}"
        );
    }
}
