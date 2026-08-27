use super::*;
use crate::editor::{
    clip_placement::ClipPlacementRejection, timeline::TimelineSerialization,
    timeline_clip::AudioClip, ulid,
};
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
    let project = TimelineSerialization::with_test_tracks();
    let audio = asset(MediaKind::Audio, true);
    let audio_rejection = validate_clip_placement(
        &project,
        ulid(1),
        audio.kind,
        TimelineTime::from_frames(30),
        TimelineTime::ZERO,
        &HashSet::new(),
    )
    .unwrap_err();
    assert_eq!(
        audio_rejection.message(),
        "Media is incompatible with the destination track"
    );

    let silent_video = asset(MediaKind::Video, false);
    let video_rejection = validate_clip_placement(
        &project,
        ulid(2),
        silent_video.kind,
        TimelineTime::from_frames(30),
        TimelineTime::ZERO,
        &HashSet::new(),
    )
    .unwrap_err();
    assert_eq!(
        video_rejection.message(),
        "Media is incompatible with the destination track"
    );
}

#[test]
fn explorer_drop_detects_collisions_but_allows_adjacent_clips() {
    let mut project = TimelineSerialization::with_test_tracks();
    project.clips.push(Clip::Audio(AudioClip {
        id: ulid(20),
        track_id: ulid(2),
        asset_id: ulid(10),
        timeline_start: TimelineTime::from_frames(30),
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::from_frames(30),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    }));
    let audio = asset(MediaKind::Audio, true);

    assert_eq!(
        validate_clip_placement(
            &project,
            ulid(2),
            audio.kind,
            TimelineTime::from_frames(30),
            TimelineTime::from_frames(15),
            &HashSet::new(),
        ),
        Err(ClipPlacementRejection::ExistingClipOverlap)
    );
    assert_eq!(
        validate_clip_placement(
            &project,
            ulid(2),
            audio.kind,
            TimelineTime::from_frames(30),
            TimelineTime::ZERO,
            &HashSet::new(),
        ),
        Ok(())
    );
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
