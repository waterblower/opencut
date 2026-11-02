//! Offscreen export rendering on Windows. GPUI only ships a headless renderer for Metal,
//! but its DirectX window renderer can already read its target back without presenting,
//! so a window that is never shown serves as the headless renderer.

use anyhow::{Context as _, Result, bail};
use gpui::{
    AnyWindowHandle, Context, DevicePixels, Empty, IntoElement, Platform, PlatformAtlas,
    PlatformHeadlessRenderer, PlatformWindow, Render, Scene, Size, Window, WindowHandle, WindowId,
    WindowKind, WindowParams, bounds, point, px, size,
};
use image::RgbaImage;
use raw_window_handle::RawWindowHandle;
use std::{rc::Rc, sync::Arc};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    UI::{
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::{
            GetClientRect, GetWindowRect, MINMAXINFO, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER,
            SetWindowPos, WM_GETMINMAXINFO,
        },
    },
};

/// Renderer factory for `HeadlessAppContext`. Call on the thread that renders.
pub(crate) fn offscreen_renderer() -> Option<Box<dyn PlatformHeadlessRenderer>> {
    match OffscreenWindowRenderer::new() {
        Ok(renderer) => Some(Box::new(renderer)),
        Err(error) => {
            // 工厂只能返回 None；导出随后以“无渲染器”失败，这里保留根因。
            eprintln!("Creating the offscreen export renderer failed: {error:?}");
            None
        }
    }
}

#[rustfmt::skip]
struct OffscreenWindowRenderer {
    window: Box<dyn PlatformWindow>, // 字段按声明顺序释放：窗口先于创建它的平台。
    _platform: Rc<dyn Platform>,     // 只为保持窗口依赖的设备与线程状态存活。
}

impl OffscreenWindowRenderer {
    fn new() -> Result<Self> {
        let platform = gpui_platform::current_platform(false);
        // 窗口从不注册到 App；句柄只交给平台窗口的回调，而这些回调从不触发。
        let handle: AnyWindowHandle =
            WindowHandle::<OffscreenRoot>::new(WindowId::from(u64::MAX)).into();
        let window = platform
            .open_window(
                handle,
                WindowParams {
                    bounds: bounds(point(px(0.0), px(0.0)), size(px(1.0), px(1.0))),
                    titlebar: None,
                    kind: WindowKind::Normal,
                    is_movable: false,
                    app_owns_titlebar_drag: false,
                    is_resizable: false,
                    is_minimizable: false,
                    focus: false,
                    show: false,
                    icon: None,
                    display_id: None,
                    app_id: None,
                    window_min_size: None,
                },
            )
            .context("Opening the offscreen export window")?;
        let renderer = Self {
            window,
            _platform: platform,
        };
        // 默认最大尺寸受屏幕限制，否则 4K 等大于屏幕的导出会被截小。
        // SAFETY: the window is live and owned by this thread; the subclass procedure is a
        // plain function that holds no state, and the window removes it when destroyed.
        unsafe { SetWindowSubclass(renderer.hwnd()?, Some(allow_offscreen_size), 1, 0) }
            .ok()
            .context("Lifting the export window's size limit")?;
        Ok(renderer)
    }

    /// Resizes the client area, and with it the DirectX target, to exactly `size`.
    /// `PlatformWindow::resize` defers to the message loop, which this thread never runs;
    /// `SetWindowPos` on the owning thread delivers `WM_SIZE` synchronously instead.
    fn resize_client(&self, size: Size<DevicePixels>) -> Result<()> {
        let hwnd = self.hwnd()?;
        let client = client_size(hwnd)?;
        if client == (size.width.0, size.height.0) {
            return Ok(());
        }
        let mut outer = RECT::default();
        // SAFETY: `hwnd` is a live window owned by this thread.
        unsafe { GetWindowRect(hwnd, &mut outer) }.context("Reading export window bounds")?;
        let border_width = (outer.right - outer.left) - client.0;
        let border_height = (outer.bottom - outer.top) - client.1;
        // SAFETY: as above; the window is never shown, so moving or activating is skipped.
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                size.width.0 + border_width,
                size.height.0 + border_height,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .context("Resizing export window")?;
        let resized = client_size(hwnd)?;
        if resized != (size.width.0, size.height.0) {
            bail!(
                "Export window is {}x{} pixels; Windows did not allow the requested {}x{}",
                resized.0,
                resized.1,
                size.width.0,
                size.height.0,
            );
        }
        Ok(())
    }

    fn hwnd(&self) -> Result<HWND> {
        let handle = match self.window.window_handle() {
            Ok(handle) => handle,
            Err(error) => bail!("Reading export window handle: {error:?}"),
        };
        match handle.as_raw() {
            RawWindowHandle::Win32(win32) => Ok(HWND(win32.hwnd.get() as _)),
            other => bail!("Export window has a non-Win32 handle: {other:?}"),
        }
    }
}

impl PlatformHeadlessRenderer for OffscreenWindowRenderer {
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> Result<RgbaImage> {
        self.resize_client(size)?;
        self.window.render_to_image(scene)
    }

    fn render_scene(&mut self, scene: &Scene, size: Size<DevicePixels>) -> Result<()> {
        self.render_scene_to_image(scene, size)?;
        Ok(())
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.window.sprite_atlas()
    }
}

/// Root type for the offscreen window handle; nothing ever renders it.
struct OffscreenRoot;

impl Render for OffscreenRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// Lets the never-shown window grow past the screen size Windows applies by default.
unsafe extern "system" fn allow_offscreen_size(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    _reference_data: usize,
) -> LRESULT {
    // SAFETY: forwards the message unchanged to the next window procedure.
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    if message == WM_GETMINMAXINFO {
        // SAFETY: for WM_GETMINMAXINFO, lparam points to the MINMAXINFO being filled in.
        let info = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
        info.ptMaxTrackSize = POINT {
            x: MAX_WINDOW_EXTENT,
            y: MAX_WINDOW_EXTENT,
        };
    }
    result
}

const MAX_WINDOW_EXTENT: i32 = 32_767; // Windows 窗口坐标上限；远大于 D3D11 的 16384 纹理上限。

fn client_size(hwnd: HWND) -> Result<(i32, i32)> {
    let mut client = RECT::default();
    // SAFETY: `hwnd` is a live window owned by this thread.
    unsafe { GetClientRect(hwnd, &mut client) }.context("Reading export window client size")?;
    Ok((client.right - client.left, client.bottom - client.top))
}
