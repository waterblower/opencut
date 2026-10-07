use crate::editor::Editor;
use crate::event_bus::AppEvent;
use crate::generic_containers;
use crate::preview_events::PreviewEvent;
use crate::timeline_interactions::TimelineTool;
use gpui::prelude::*;
use gpui::{App, KeyBinding, Window, actions};

pub const EDITOR_KEY_CONTEXT: &str = "Editor";
pub const EDITOR_SHORTCUT_CONTEXT: &str = "Editor && !TextInput";

actions!(
    opencut_editor,
    [
        TogglePlayback,
        StepBackwardFrame,
        StepForwardFrame,
        DeleteSelected,
        SplitClip,
        Undo,
        Redo,
        DuplicateSelected,
        CopySelectedClips,
        CutSelectedClips,
        PasteClips,
        SelectAllUnlockedClips,
        SelectClipsRightOfPointer,
        ActivateSelectionTool,
        ActivateBladeTool,
        ToggleFullscreen,
        ExitFullscreen,
        ToggleInspector,
        RevealInFinder,
        OpenInDefaultApp
    ]
);

pub(crate) fn bind_keys(cx: &mut App) {
    generic_containers::text_input::bind_keys(cx);
    cx.bind_keys([
        KeyBinding::new("space", TogglePlayback, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("left", StepBackwardFrame, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("right", StepForwardFrame, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("backspace", DeleteSelected, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("delete", DeleteSelected, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("cmd-b", SplitClip, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("cmd-z", Undo, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("cmd-shift-z", Redo, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("cmd-d", DuplicateSelected, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("cmd-c", CopySelectedClips, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("cmd-x", CutSelectedClips, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("cmd-v", PasteClips, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new(
            "cmd-a",
            SelectAllUnlockedClips,
            Some(EDITOR_SHORTCUT_CONTEXT),
        ),
        KeyBinding::new(
            "]",
            SelectClipsRightOfPointer,
            Some(EDITOR_SHORTCUT_CONTEXT),
        ),
        KeyBinding::new("v", ActivateSelectionTool, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("b", ActivateBladeTool, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("f", ToggleFullscreen, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("escape", ExitFullscreen, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("cmd-alt-i", ToggleInspector, None),
        KeyBinding::new("cmd-alt-r", RevealInFinder, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new(
            "ctrl-shift-enter",
            OpenInDefaultApp,
            Some(EDITOR_SHORTCUT_CONTEXT),
        ),
    ]);
    cx.on_action::<ToggleInspector>(|_, cx| {
        let Some(window) = cx.active_window() else {
            return;
        };
        let _ = window.update(cx, |_, window, cx| {
            crate::gpui_inspector::toggle(window, cx)
        });
    });
}

impl Editor {
    pub(crate) fn action_toggle_playback(
        &mut self,
        _: &TogglePlayback,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.emit_event(cx, AppEvent::Preview(PreviewEvent::TogglePlayback));
        cx.notify();
    }

    pub(crate) fn action_step_backward_frame(
        &mut self,
        _: &StepBackwardFrame,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.step_playhead(-1) {
            log::error!("{error:?}");
        }
        cx.notify();
    }

    pub(crate) fn action_step_forward_frame(
        &mut self,
        _: &StepForwardFrame,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.step_playhead(1) {
            log::error!("{error:?}");
        }
        cx.notify();
    }

    pub(crate) fn action_delete_selected(
        &mut self,
        _: &DeleteSelected,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.delete_selected() {
            log::error!("{error:?}");
        }
        cx.notify();
    }

    pub(crate) fn action_split_clip(
        &mut self,
        _: &SplitClip,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        if let Err(error) = timeline.blade_at_playhead() {
            log::error!("{error:?}");
        }
        cx.notify();
    }

    pub(crate) fn action_undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = self.undo() {
            log::error!("{error:?}");
        }
        cx.notify();
    }

    pub(crate) fn action_redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = self.redo() {
            log::error!("{error:?}");
        }
        cx.notify();
    }

    pub(crate) fn action_duplicate_selected(
        &mut self,
        _: &DuplicateSelected,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.duplicate_selected() {
            log::error!("{error:?}");
        }
        cx.notify();
    }

    pub(crate) fn action_copy_selected_clips(
        &mut self,
        _: &CopySelectedClips,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.copy_selected_clips();
        cx.notify();
    }

    pub(crate) fn action_cut_selected_clips(
        &mut self,
        _: &CutSelectedClips,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.cut_selected_clips() {
            log::error!("{error:?}");
        }
        cx.notify();
    }

    pub(crate) fn action_paste_clips(
        &mut self,
        _: &PasteClips,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.paste_clips(cx) {
            log::error!("{error:?}");
        }
        cx.notify();
    }

    pub(crate) fn action_select_clips_right_of_pointer(
        &mut self,
        _: &SelectClipsRightOfPointer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let pointer = window.mouse_position();
        let bounds = timeline.h_scroll.bounds();
        if !bounds.contains(&pointer) {
            return;
        }
        let content_x = f32::from(pointer.x - bounds.left() - timeline.h_scroll.offset().x)
            - crate::layout::TIMELINE_PADDING;
        let cursor_seconds = content_x.max(0.0) as f64 / timeline.pixels_per_second as f64;
        timeline.interaction.selected_clip_ids = timeline
            .editing_state
            .clips
            .iter()
            .filter(|clip| {
                timeline
                    .editing_state
                    .seconds(clip.timeline_end(timeline.editing_state.settings.frame_rate))
                    > cursor_seconds
                    && timeline
                        .editing_state
                        .track(clip.track_id())
                        .is_some_and(|track| !track.locked)
            })
            .map(|clip| clip.id())
            .collect();
        timeline.interaction.selected_clip_id = timeline
            .editing_state
            .clips
            .iter()
            .filter(|clip| timeline.interaction.selected_clip_ids.contains(&clip.id()))
            .min_by_key(|clip| clip.timeline_start())
            .map(|clip| clip.id());
        self.properties.transform_input_clip_id = None;
        self.properties.text_input_clip_id = None;
        cx.notify();
    }

    pub(crate) fn action_select_all_unlocked_clips(
        &mut self,
        _: &SelectAllUnlockedClips,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_all_unlocked_clips();
        cx.notify();
    }

    pub(crate) fn action_activate_selection_tool(
        &mut self,
        _: &ActivateSelectionTool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.activate_timeline_tool(TimelineTool::Selection);
        cx.notify();
    }

    pub(crate) fn action_activate_blade_tool(
        &mut self,
        _: &ActivateBladeTool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.activate_timeline_tool(TimelineTool::Blade);
        cx.notify();
    }

    pub(crate) fn action_toggle_fullscreen(
        &mut self,
        _: &ToggleFullscreen,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preview.fullscreen = !self.preview.fullscreen;
        cx.notify();
    }

    pub(crate) fn action_exit_fullscreen(
        &mut self,
        _: &ExitFullscreen,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.preview.fullscreen {
            self.preview.fullscreen = false;
            cx.notify();
        }
    }

    pub(crate) fn action_toggle_inspector(
        &mut self,
        _: &ToggleInspector,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        crate::gpui_inspector::toggle(window, cx);
    }
}
