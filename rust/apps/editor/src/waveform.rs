use crate::editor::Editor;
use crate::event_bus::AppEvent;
use crate::timeline_document::{project_timeline_files, resolve_asset_path};
use ::timeline::TimelineSerialization;
use gpui::Context;
use std::collections::HashSet;

impl Editor {
    pub(super) fn schedule_project_waveforms(&mut self, cx: &mut Context<Self>) {
        let mut paths = HashSet::new();
        let timeline_paths = match project_timeline_files(&self.project_root) {
            Ok(paths) => paths,
            Err(error) => {
                eprintln!("Could not scan project timelines for waveforms: {error}");
                return;
            }
        };
        for timeline_path in timeline_paths {
            let timeline_file = self.project_root.join(&timeline_path);
            let timeline = match TimelineSerialization::load(&timeline_file) {
                Ok(timeline) => timeline,
                Err(error) => {
                    eprintln!("Could not scan timeline for waveforms: {error}");
                    continue;
                }
            };
            let Some(timeline_directory) = timeline_file.parent() else {
                continue;
            };
            let editing_state = timeline.editing_state;
            let referenced_assets = editing_state
                .clips
                .iter()
                .filter_map(|clip| clip.asset_id())
                .collect::<HashSet<_>>();
            paths.extend(
                editing_state
                    .assets
                    .into_iter()
                    .filter(|asset| asset.has_audio() && referenced_assets.contains(&asset.id()))
                    .map(|asset| resolve_asset_path(timeline_directory, &asset.path())),
            );
        }
        self.emit_event(
            cx,
            AppEvent::WaveformsGeneration {
                paths: paths.into_iter().collect(),
            },
        );
    }

    pub(super) fn schedule_active_timeline_waveforms(&mut self, cx: &mut Context<Self>) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let Some(timeline_directory) = timeline.path.parent() else {
            return;
        };
        let referenced_assets = timeline
            .editing_state
            .clips
            .iter()
            .filter_map(|clip| clip.asset_id())
            .collect::<HashSet<_>>();
        let paths = timeline
            .editing_state
            .assets
            .iter()
            .filter(|asset| asset.has_audio() && referenced_assets.contains(&asset.id()))
            .map(|asset| resolve_asset_path(timeline_directory, &asset.path()))
            .collect::<Vec<_>>();
        self.emit_event(cx, AppEvent::WaveformsGeneration { paths });
    }
}
