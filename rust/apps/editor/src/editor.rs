use crate::context_menu::ContextMenu;
use crate::dialog_missing_media::MissingMediaDialog;
use crate::editing::ClipClipboard;
use crate::event_bus::{AppEvent, EventBus};
use crate::explorer::ExplorerState;
use crate::explorer_drag::AssetBeingDragged;
use crate::preview::{PreviewState, PreviewTarget};
use crate::project_settings::load_project_local_settings;
use crate::property_panel::{PropertyPanel, PropertyPanelState, current_property_panel};
use crate::property_panel_transform::VideoTransformInputs;
use crate::timeline::TimelineRuntimeState;
use crate::ui_components::{HorizontalSplitState, TextInput, VerticalSplitState};
use crate::waveform;
use anyhow::Result;
use gpui::prelude::*;
use gpui::{Entity, FocusHandle, ScrollHandle};
use std::collections::{HashMap, HashSet};
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) struct Editor {
    // main UI sections
    pub(super) explorer: ExplorerState,
    pub(super) preview: PreviewState,
    pub(crate) timeline: Option<TimelineRuntimeState>,

    // other
    pub(crate) project_root: PathBuf,
    pub(super) waveform_jobs: HashSet<PathBuf>, // 正在生成波形的素材绝对路径。
    pub(super) waveform_cache: HashMap<PathBuf, Arc<waveform::WaveformData>>, // 键为素材的绝对路径；不同目录的时间线可能用不同相对路径指向同一文件。
    pub(super) property_panel_state: PropertyPanelState,
    pub(super) property_panel: PropertyPanel, // 点击文件或片段后由事件重建，render 只读它。
    pub(super) settings_open: bool,

    pub(super) focus_handle: FocusHandle,
    pub(super) clipboard: Option<ClipClipboard>,
    pub(super) context_menu: ContextMenu,
    pub(crate) active_asset_drag: AssetBeingDragged,
    // entities
    pub(super) event_bus: Entity<EventBus>,
    pub(super) upper_split_state: Entity<HorizontalSplitState>,
    pub(super) workspace_split_state: Entity<VerticalSplitState>,
    pub(crate) global_settings_input: Option<Entity<TextInput>>,
    pub(crate) missing_media_dialog: Option<MissingMediaDialog>,
}

impl Editor {
    pub(crate) fn new(
        project_root: PathBuf,                  // 调用方已转成绝对路径
        timeline: Option<TimelineRuntimeState>, // 调用方已加载好
        event_bus: Entity<EventBus>,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let explorer = ExplorerState::new(
            &project_root,
            timeline.as_ref().and_then(|timeline| {
                timeline
                    .path
                    .strip_prefix(&project_root)
                    .ok()
                    .map(Path::to_path_buf)
            }),
            focus_handle.clone(),
            cx,
        );

        let preview = PreviewState {
            target: PreviewTarget::None, // 实体创建后由调用方设置
            fullscreen: false,
            error_scroll: ScrollHandle::new(),
        };

        let property_panel_state = {
            let video_transform_inputs = VideoTransformInputs::new(focus_handle.clone(), cx);
            Self::observe_video_transform_inputs(&video_transform_inputs, cx);

            PropertyPanelState {
                transform_inputs: video_transform_inputs,
                transform_input_clip_id: None,
                text_input_clip_id: None,
            }
        };

        let project_local_settings = load_project_local_settings(&project_root);
        let mut editor = Self {
            // Entities
            event_bus,
            upper_split_state: cx.new(|_| project_local_settings.upper_space_split_state),
            workspace_split_state: cx.new(|_| project_local_settings.workspace_split_state),
            //
            project_root,
            explorer,
            preview,
            waveform_jobs: HashSet::new(),
            waveform_cache: HashMap::new(),
            property_panel_state,
            property_panel: PropertyPanel::None,
            settings_open: false,
            global_settings_input: None,
            missing_media_dialog: None,
            timeline,
            clipboard: None,

            focus_handle,
            context_menu: ContextMenu::None,
            active_asset_drag: AssetBeingDragged::None,
        };
        if let Some(timeline) = editor.timeline.as_mut() {
            timeline.set_playhead(timeline.playhead());
        }
        editor.property_panel = current_property_panel(&editor);
        editor
    }

    pub(crate) fn emit_event(&mut self, cx: &mut Context<Self>, event: AppEvent) {
        self.event_bus.update(cx, |_, cx| cx.emit(event));
    }
}

/// 读取项目设置中的 active timeline；文件已被移走或删除时返回 None。
pub(crate) fn load_active_timeline(project_root: &Path) -> Result<Option<TimelineRuntimeState>> {
    let project_settings = load_project_local_settings(project_root);
    let Some(timeline_path) = project_settings.active_timeline else {
        return Ok(None);
    };
    match TimelineRuntimeState::load(project_root.join(&timeline_path)) {
        Ok(timeline) => Ok(Some(timeline)),
        Err(error) => {
            // The saved timeline may have been moved or deleted; open
            // the editor without an active timeline in that case.
            if error
                .downcast_ref::<IoError>()
                .is_some_and(|error| error.kind() == ErrorKind::NotFound)
            {
                return Ok(None);
            }
            // We still error out for other kinds of errors.
            Err(error)
        }
    }
}
