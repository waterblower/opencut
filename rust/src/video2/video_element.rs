//! GPUI rendering for playback snapshots. A stable element ID enables caching.

use std::sync::Arc;

use anyhow::{Context as _, Result};
use gpui::{
    App, AppContext, Bounds, Element, ElementId, Entity, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Window, point, px, size,
};

use super::{VideoBackend, VideoFrame};

mod conversion;
use conversion::{Converter, Prepared};

/// Captures the current frame without advancing playback. Backend errors are
/// returned here; conversion/painting errors are handled at the paint boundary.
pub fn video(backend: &VideoBackend) -> Result<VideoElement> {
    let frame = backend.get_current_frame()?;
    Ok(VideoElement {
        width: px(frame.image.width() as f32),
        height: px(frame.image.height() as f32),
        frame,
        id: None,
    })
}

pub struct VideoElement {
    pub frame: Arc<VideoFrame>,
    pub width: Pixels,
    pub height: Pixels,
    pub id: Option<ElementId>,
}

impl VideoElement {
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn size(mut self, width: Pixels, height: Pixels) -> Self {
        self.width = width;
        self.height = height;
        self
    }
}

/// Holds the cache through the paint phase, including for unidentified elements.
pub struct VideoPrepaintState {
    cache: Option<Entity<Cache>>,
}

impl Element for VideoElement {
    type RequestLayoutState = ();
    type PrepaintState = VideoPrepaintState;

    fn id(&self) -> Option<ElementId> {
        self.id.clone()
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let style = gpui::Style {
            size: size(self.width.into(), self.height.into()),
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> VideoPrepaintState {
        // Playback is autonomous and does not notify GPUI. The next parent
        // render calls video() again to capture the latest snapshot.
        window.request_animation_frame();
        if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
            return VideoPrepaintState { cache: None };
        }
        let cache = match global_id {
            Some(id) => window.with_element_state(id, |previous: Option<Entity<Cache>>, _| {
                let cache = match previous {
                    Some(cache) => cache,
                    None => new_cache(cx),
                };
                (cache.clone(), cache)
            }),
            None => new_cache(cx),
        };
        VideoPrepaintState { cache: Some(cache) }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        state: &mut VideoPrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(cache) = &state.cache else {
            return;
        };
        cache.update(cx, |cache, _| {
            if let Err(error) = cache.paint(&self.frame, bounds, window) {
                // GPUI's Element::paint cannot return errors. Failed snapshots
                // remain cached so an identified element reports each one once.
                log::error!("Could not paint video: {error:#}");
            }
        });
    }
}

impl IntoElement for VideoElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

#[derive(Default)]
struct Cache {
    converter: Converter,
    entry: Option<CachedFrame>,
}

struct CachedFrame {
    source: Arc<VideoFrame>,
    // None records a failed conversion/paint, rather than a pending operation.
    prepared: Option<Prepared>,
}

impl Cache {
    fn paint(
        &mut self,
        frame: &Arc<VideoFrame>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
    ) -> Result<()> {
        let unchanged = match &self.entry {
            Some(entry) => Arc::ptr_eq(&entry.source, frame),
            None => false,
        };
        if !unchanged {
            let previous = self.entry.replace(CachedFrame {
                source: Arc::clone(frame),
                prepared: None,
            });
            if let Some(previous) = previous {
                release_image(previous.prepared, window)?;
            }
            let prepared = self.converter.prepare(&frame.image)?;
            self.entry
                .as_mut()
                .expect("entry was just installed")
                .prepared = Some(prepared);
        }
        let Some(entry) = self.entry.as_mut() else {
            return Ok(());
        };
        let Some(prepared) = &entry.prepared else {
            return Ok(());
        };
        let image_bounds = fitted_bounds(bounds, frame.image.width(), frame.image.height());
        let result = match prepared {
            #[cfg(target_os = "macos")]
            Prepared::Surface(surface) => {
                window.paint_surface(image_bounds, surface.clone());
                Ok(())
            }
            Prepared::Image(image) => window
                .paint_image(
                    bounds,
                    image_bounds,
                    gpui::Corners::default(),
                    Arc::clone(image),
                    0,
                    false,
                )
                .context(format!("Painting video image at {}:{}", file!(), line!())),
        };
        if result.is_err() {
            release_image(entry.prepared.take(), window)?;
        }
        result
    }
}

fn new_cache(cx: &mut App) -> Entity<Cache> {
    cx.new(|cx| {
        // GPUI flushes release effects after drawing. This also cleans up the
        // final atlas entry when an element disappears or has no persistent ID.
        cx.on_release(|cache: &mut Cache, cx| {
            if let Some(entry) = cache.entry.take()
                && let Some(Prepared::Image(image)) = entry.prepared
            {
                cx.drop_image(image, None);
            }
        })
        .detach();
        Cache::default()
    })
}

fn release_image(prepared: Option<Prepared>, window: &mut Window) -> Result<()> {
    if let Some(Prepared::Image(image)) = prepared {
        window.drop_image(image).context(format!(
            "Releasing video image at {}:{}",
            file!(),
            line!()
        ))?;
    }
    Ok(())
}

fn fitted_bounds(bounds: Bounds<Pixels>, width: u32, height: u32) -> Bounds<Pixels> {
    if width == 0 || height == 0 || bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
        return Bounds::new(bounds.origin, size(px(0.0), px(0.0)));
    }
    let scale = (f32::from(bounds.size.width) / width as f32)
        .min(f32::from(bounds.size.height) / height as f32);
    let fitted = size(px(width as f32 * scale), px(height as f32 * scale));
    Bounds::new(
        point(
            bounds.origin.x + (bounds.size.width - fitted.width) / 2.0,
            bounds.origin.y + (bounds.size.height - fitted.height) / 2.0,
        ),
        fitted,
    )
}

#[cfg(test)]
#[path = "video_element.test.rs"]
mod tests;

#[cfg(all(test, feature = "ffmpeg-video-tests"))]
#[path = "video_element.gpui_test.rs"]
mod gpui_tests;
