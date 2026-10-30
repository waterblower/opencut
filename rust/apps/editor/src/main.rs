#![allow(dead_code)]

mod actions;
mod asset;
mod clip_placement;
mod context_menu;
mod debug_state;
mod dialog_missing_media;
mod edit_action;
mod editing;
mod editor;
mod editor_view;
mod event_bus;
mod explorer;
mod explorer_drag;
mod explorer_file_entry;
mod explorer_file_menu;
mod explorer_view;
mod export_window;
mod global_settings;
mod global_settings_dialog;
mod gpui_inspector;
mod layout;
mod preview;
mod preview_events;
mod preview_image;
mod preview_text;
mod project;
mod project_picker;
mod project_settings;
mod property_panel;
mod property_panel_audio_clip;
mod property_panel_audio_file;
mod property_panel_empty;
mod property_panel_image_file;
mod property_panel_srt_file;
mod property_panel_text;
mod property_panel_timeline;
mod property_panel_transform;
mod property_panel_video_clip;
mod property_panel_video_file;
mod settings;
mod srt;
#[cfg(test)]
#[path = "tests/test_support.rs"]
mod test_support;
mod theme;
mod time_format;
mod timeline;
mod timeline_clip;
mod timeline_clip_menu;
mod timeline_document;
mod timeline_interactions;
mod timeline_persistence;
mod timeline_track_menu;
mod timeline_trim;
mod timeline_ui;
mod track;
mod track_ui;
mod transcription_window;
#[path = "ui-components/mod.rs"]
mod ui_components;
mod waveform;

use crate::preview::timeline_preview_target;
use anyhow::{Context as _, Result};
use asset::EditorAssets;
use editor::{Editor, load_active_timeline};
use event_bus::{EventBus, handle_event};
use global_settings::GlobalEditorSettings;
use gpui::{
    App, Bounds, Entity, Subscription, WindowBounds, WindowHandle, WindowOptions, prelude::*, px,
    size,
};
use gpui_platform::application;
use preview::PreviewTarget;
use std::io::Write as _;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use {
    objc2_app_kit::{NSApplication, NSImage},
    objc2_foundation::{MainThreadMarker, NSData},
};

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("editor=debug"))
        .format(|buffer, record| {
            writeln!(
                buffer,
                "{}\n\t\t[{}:{}]",
                record.args(),
                record.file().unwrap_or("<unknown>"),
                record.line().unwrap_or(0)
            )
        })
        .init();

    let settings = GlobalEditorSettings::load()?;

    application().with_assets(EditorAssets).run(move |cx| {
        #[cfg(target_os = "macos")]
        if let Err(error) = set_dock_icon() {
            log::error!("Could not set Dock icon: {error:?}");
            cx.quit();
            return;
        }

        gpui_tokio::init(cx);
        gpui_inspector::init(cx);
        actions::bind_keys(cx);
        cx.set_quit_mode(gpui::QuitMode::Explicit);

        let event_bus = cx.new(|_| EventBus {});

        // Restore the previous project when its folder is still available.
        if settings.project_root.is_dir() {
            if let Err(error) = open_project(cx, settings.project_root, event_bus) {
                log::error!("{error:?}");
                cx.quit();
            }
            return;
        }
        let missing_project = if settings.project_root.as_os_str().is_empty() {
            None
        } else {
            Some(settings.project_root)
        };
        if let Err(error) = project_picker::open(cx, event_bus, missing_project) {
            log::error!("Could not open project picker: {error:?}");
            cx.quit();
        }
    });
    Ok(())
}

/// The open editor window, and the subscription that quits the app when its last window closes.
struct OpenProject {
    window: WindowHandle<Editor>,
    close_subscription: Option<Subscription>,
}

fn open_project(cx: &mut App, project_root: PathBuf, event_bus: Entity<EventBus>) -> Result<()> {
    let window = open_editor_window(project_root, event_bus.clone(), cx)?;

    let close_subscription = cx.on_window_closed(quit_after_last_window);
    let project = cx.new(|_| OpenProject {
        window,
        close_subscription: Some(close_subscription),
    });
    cx.subscribe(&event_bus, move |event_bus, event, cx| {
        let project_for_task = project.clone();
        let owned_event = event.clone();
        cx.spawn(async move |cx| handle_event(cx, project_for_task, event_bus, owned_event).await)
            .detach();
    })
    .detach();
    // 订阅建立之后再触发，否则 WaveformsNeeded 事件没有处理者。
    window.update(cx, |editor, _, cx| editor.schedule_project_waveforms(cx))?;
    Ok(())
}

fn quit_after_last_window(cx: &mut App, _: gpui::WindowId) {
    if cx.windows().is_empty() {
        cx.quit();
    }
}

fn open_editor_window(
    root: PathBuf,
    event_bus: Entity<EventBus>,
    cx: &mut App,
) -> Result<WindowHandle<Editor>> {
    let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);

    // 可能失败、且不需要 cx 的准备工作，放在 cx.new 之前
    let project_root = std::path::absolute(&root)?;
    let timeline = load_active_timeline(&project_root)?;
    let editor = cx.new(|cx| Editor::new(project_root, timeline, event_bus, cx));

    // 预览需要 Context<Editor> 而且可能失败，所以在实体创建之后设置
    editor.update(cx, |editor, cx| -> Result<()> {
        editor.preview.target = match editor.timeline.as_ref() {
            Some(timeline) => {
                timeline_preview_target(timeline, &editor.project_root, cx).context(format!(
                    "creating preview for timeline '{}'",
                    timeline.path.display()
                ))?
            }
            None => PreviewTarget::None,
        };
        Ok(())
    })?;

    let window = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..WindowOptions::default()
            },
            |window, cx| {
                editor.read(cx).focus_handle.clone().focus(window, cx);
                editor
            },
        )
        .context(format!("creating editor window for {}", root.display()))?;
    cx.activate(true);
    Ok(window)
}

#[cfg(target_os = "macos")]
fn set_dock_icon() -> Result<()> {
    let main_thread = MainThreadMarker::new().context("Dock icon requires the main thread")?;
    let tree = resvg::usvg::Tree::from_data(
        include_bytes!("../icons/app-icon.svg"),
        &resvg::usvg::Options::default(),
    )
    .context("parsing the application icon SVG")?;
    let size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .context("allocating the application icon pixels")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let png = pixmap
        .encode_png()
        .context("encoding the application icon")?;
    let data = NSData::from_vec(png);
    let image = NSImage::initWithData(main_thread.alloc(), &data)
        .context("creating the application icon image")?;
    let application = NSApplication::sharedApplication(main_thread);
    // SAFETY: 在主线程传入有效的非空 NSImage；AppKit 持有设置后的图标。
    unsafe { application.setApplicationIconImage(Some(&image)) };
    Ok(())
}
