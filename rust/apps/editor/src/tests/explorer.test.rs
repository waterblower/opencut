use super::*;
use crate::clip_placement::ClipPlacementRejection;
use crate::explorer_drag::{AssetBeingDragged, AssetBeingDraggedV1};
use crate::model::MediaKind;
use crate::test_support::{TimelineTestExt, ulid};
use crate::timeline::PreviewDropAsset;
use crate::timeline_clip::AudioClip;
use ::timeline::TimelineEditingState;
use std::path::Path;

fn asset(kind: MediaKind, has_audio: bool) -> MediaAsset {
    MediaAsset {
        id: ulid(10),
        kind,
        path: PathBuf::from("media.mp4"),
        name: "Media".to_string(),
        duration: 10.0,
        width: 1920,
        height: 1080,
        framerate: 30.0,
        frame_rate_numerator: 30,
        frame_rate_denominator: 1,
        codec: "test".to_string(),
        has_audio,
    }
}

#[test]
fn explorer_drop_rejects_incompatible_tracks() {
    let project = TimelineEditingState::with_test_tracks();
    let mut audio_preview = PreviewDropAsset {
        track_id: ulid(1),
        start_time: TimelineFrameIndex::ZERO,
        asset: AssetBeingDragged::V1(AssetBeingDraggedV1 {
            absolute_path: PathBuf::from("audio.wav"),
            metadata: asset(MediaKind::Audio, true),
        }),
    };
    let audio_rejection = audio_preview
        .validate_placement(&project)
        .unwrap_err()
        .downcast::<ClipPlacementRejection>()
        .unwrap();
    assert_eq!(audio_rejection.message(), "Incompatible track");

    let video_preview = PreviewDropAsset {
        track_id: ulid(2),
        start_time: TimelineFrameIndex::ZERO,
        asset: AssetBeingDragged::V1(AssetBeingDraggedV1 {
            absolute_path: PathBuf::from("silent.mp4"),
            metadata: asset(MediaKind::Video, false),
        }),
    };
    let video_rejection = video_preview
        .validate_placement(&project)
        .unwrap_err()
        .downcast::<ClipPlacementRejection>()
        .unwrap();
    assert_eq!(video_rejection.message(), "Incompatible track");

    // The same preview becomes valid as soon as its destination is compatible.
    audio_preview.track_id = ulid(2);
    assert!(audio_preview.validate_placement(&project).is_ok());
    let mut locked_project = project.clone();
    locked_project
        .tracks
        .iter_mut()
        .find(|track| track.id == ulid(2))
        .unwrap()
        .locked = true;
    assert_eq!(
        audio_preview
            .validate_placement(&locked_project)
            .unwrap_err()
            .downcast::<ClipPlacementRejection>()
            .unwrap(),
        ClipPlacementRejection::LockedTrack,
    );
}

#[test]
fn explorer_drop_detects_collisions_but_allows_adjacent_clips() {
    let mut project = TimelineEditingState::with_test_tracks();
    project.clips.push(Clip::Audio(AudioClip::new(
        ulid(20),
        ulid(2),
        ulid(10),
        TimelineFrameIndex::from(30),
        TimelineFrameIndex::ZERO,
        TimelineFrameIndex::from(30),
        AudioClipProperties::default(),
    )));
    let mut preview = PreviewDropAsset {
        track_id: ulid(2),
        start_time: TimelineFrameIndex::from(15),
        asset: AssetBeingDragged::V1(AssetBeingDraggedV1 {
            absolute_path: PathBuf::from("audio.wav"),
            metadata: MediaAsset {
                duration: 1.0,
                ..asset(MediaKind::Audio, true)
            },
        }),
    };
    assert_eq!(
        preview
            .validate_placement(&project)
            .unwrap_err()
            .downcast::<ClipPlacementRejection>()
            .unwrap(),
        ClipPlacementRejection::ExistingClipOverlap,
    );
    preview.start_time = TimelineFrameIndex::ZERO;
    assert!(preview.validate_placement(&project).is_ok());
}

#[test]
fn renamed_path_stays_in_the_same_directory() {
    assert_eq!(
        renamed_relative_path(Path::new("media/old.mp4"), "new.mp4"),
        Some(PathBuf::from("media/new.mp4"))
    );
    assert_eq!(
        renamed_relative_path(Path::new("old.mp4"), "../new.mp4"),
        None
    );
}

#[test]
fn directory_rename_remaps_descendants() {
    assert_eq!(
        remap_relative_path(
            Path::new("old/nested/clip.mp4"),
            Path::new("old"),
            Path::new("new")
        ),
        Some(PathBuf::from("new/nested/clip.mp4"))
    );
}

#[test]
fn explorer_expansion_round_trips_in_the_project_directory() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let project_root = std::env::temp_dir().join(format!("opencut-explorer-state-{unique}"));
    std::fs::create_dir_all(&project_root).unwrap();
    let expanded_directories =
        HashSet::from([PathBuf::from("media"), PathBuf::from("timelines/drafts")]);

    save_explorer_expansion(&project_root, &expanded_directories, false).unwrap();
    let restored = load_explorer_expansion(&project_root);

    assert_eq!(restored.expanded_directories, expanded_directories);
    assert!(!restored.root_expanded);
    assert!(project_root.join(".opencut/file-explorer.json").is_file());
    std::fs::remove_dir_all(project_root).unwrap();
}

#[test]
fn explorer_expansion_defaults_to_an_open_root() {
    let project_root = std::env::temp_dir().join(format!(
        "opencut-missing-explorer-state-{}",
        std::process::id()
    ));
    let restored = load_explorer_expansion(&project_root);

    assert!(restored.expanded_directories.is_empty());
    assert!(restored.root_expanded);
}
