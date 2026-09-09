use super::*;
use gpui::{Context, Render, RenderImage, TestAppContext, VisualTestContext, prelude::*};
use std::{cell::RefCell, rc::Rc};

#[gpui::test]
fn identity_cache_reuses_resize_and_releases_replaced_and_final_images(cx: &mut TestAppContext) {
    let window = cx.add_empty_window();
    let source = tests::rgb_frame(67, 35, [255, 0, 0]);
    let image = Converter::default().bgra(&source.image).unwrap();
    let view = window.new(|_| Probe::new(source.clone(), Some("video")));
    view.update(window, |view, _| view.seed = Some(image.clone()));
    let first = draw(window, &view).unwrap();
    window.update(|window, _| assert!(window.has_image_atlas_entry(&image)));
    view.update(window, |view, _| {
        view.dimensions = size(px(300.0), px(200.0))
    });
    let second = draw(window, &view).unwrap();
    assert_eq!(first.entity_id(), second.entity_id());
    window.update(|_, cx| {
        let Some(Prepared::Image(cached)) = &second.read(cx).entry.as_ref().unwrap().prepared
        else {
            panic!("identical frame was unnecessarily converted");
        };
        assert!(Arc::ptr_eq(cached, &image));
    });
    let replacement = tests::rgb_frame(67, 35, [0, 255, 0]);
    assert_eq!(source.timestamp, replacement.timestamp);
    view.update(window, |view, _| view.source = replacement.clone());
    let third = draw(window, &view).unwrap();
    window.update(|window, cx| {
        assert!(!window.has_image_atlas_entry(&image));
        assert!(Arc::ptr_eq(
            &third.read(cx).entry.as_ref().unwrap().source,
            &replacement
        ));
    });
    // Force the portable path on macOS too, including final atlas cleanup.
    let final_image = Converter::default().bgra(&replacement.image).unwrap();
    view.update(window, |view, _| view.seed = Some(final_image.clone()));
    let fourth = draw(window, &view).unwrap();
    window.update(|window, _| assert!(window.has_image_atlas_entry(&final_image)));
    drop((first, second, third, fourth, view));
    window.run_until_parked();
    window.update(|window, _| window.refresh()); // Render the empty root to retire element state.
    window.update(|_, _| {}); // Flush entity-release effects before inspecting the atlas.
    window.update(|window, _| assert!(!window.has_image_atlas_entry(&final_image)));
}

#[gpui::test]
fn separate_ids_do_not_share_cache_and_failures_are_cached(cx: &mut TestAppContext) {
    let window = cx.add_empty_window();
    let source = tests::rgb_frame(66, 34, [255, 0, 0]);
    let view = window.new(|_| Probe::new(source, Some("first")));
    let first = draw(window, &view).unwrap();
    view.update(window, |view, _| view.id = Some("second"));
    let second = draw(window, &view).unwrap();
    assert_ne!(first.entity_id(), second.entity_id());
    let broken = Arc::new(VideoFrame {
        timestamp: std::time::Duration::ZERO,
        image: ffmpeg_next::frame::Video::empty(),
    });
    window.draw(
        point(px(0.0), px(0.0)),
        size(px(100.0), px(100.0)),
        |_, _| {
            gpui::canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let mut cache = Cache::default();
                    assert!(cache.paint(&broken, bounds, window).is_err());
                    assert!(cache.paint(&broken, bounds, window).is_ok());
                    assert!(cache.entry.as_ref().unwrap().prepared.is_none());
                },
            )
            .size_full()
            .into_any_element()
        },
    );
}

#[gpui::test]
fn unidentified_and_zero_sized_elements_have_bounded_lifetimes(cx: &mut TestAppContext) {
    let window = cx.add_empty_window();
    let source = tests::rgb_frame(66, 34, [255, 0, 0]);
    let image = Converter::default().bgra(&source.image).unwrap();
    let view = window.new(|_| Probe::new(source, None));
    view.update(window, |view, _| view.seed = Some(image.clone()));
    let first = draw(window, &view).unwrap();
    window.update(|window, _| assert!(window.has_image_atlas_entry(&image)));
    let weak = first.downgrade();
    drop(first);
    window.run_until_parked();
    window.update(|_, _| {});
    assert!(weak.upgrade().is_none());
    window.update(|window, _| assert!(!window.has_image_atlas_entry(&image)));
    view.update(window, |view, _| view.dimensions = size(px(0.0), px(0.0)));
    assert!(draw(window, &view).is_none());
}

// A real view supplies the reactive boundary required by animation requests.
// The canvas exposes prepaint state to tests without changing production APIs.
struct Probe {
    source: Arc<VideoFrame>,
    id: Option<&'static str>,
    dimensions: gpui::Size<Pixels>,
    seed: Option<Arc<RenderImage>>,
    observed: Rc<RefCell<Option<Entity<Cache>>>>,
}

impl Probe {
    fn new(source: Arc<VideoFrame>, id: Option<&'static str>) -> Self {
        Self {
            source,
            id,
            dimensions: size(px(200.0), px(100.0)),
            seed: None,
            observed: Rc::default(),
        }
    }
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut element = VideoElement {
            frame: self.source.clone(),
            id: self.id.map(ElementId::from),
            width: self.dimensions.width,
            height: self.dimensions.height,
        };
        let seed = self.seed.take();
        let observed = self.observed.clone();
        gpui::canvas(
            move |bounds, window, cx| {
                let state = match element.id.clone() {
                    Some(id) => window.with_global_id(id, |id, window| {
                        element.prepaint(Some(id), None, bounds, &mut (), window, cx)
                    }),
                    None => element.prepaint(None, None, bounds, &mut (), window, cx),
                };
                (element, state, seed)
            },
            move |bounds, (mut element, mut state, seed), window, cx| {
                if let Some(image) = seed {
                    state.cache.as_ref().unwrap().update(cx, |cache, _| {
                        if let Some(previous) = cache.entry.take() {
                            release_image(previous.prepared, window).unwrap();
                        }
                        cache.entry = Some(CachedFrame {
                            source: element.frame.clone(),
                            prepared: Some(Prepared::Image(image)),
                        });
                    });
                }
                element.paint(None, None, bounds, &mut (), &mut state, window, cx);
                *observed.borrow_mut() = state.cache;
            },
        )
        .w(self.dimensions.width)
        .h(self.dimensions.height)
    }
}

fn draw(window: &mut VisualTestContext, view: &Entity<Probe>) -> Option<Entity<Cache>> {
    window.draw(
        point(px(0.0), px(0.0)),
        size(px(300.0), px(200.0)),
        |_, _| view.clone().into_element(),
    );
    view.update(window, |view, _| view.observed.borrow_mut().take())
}
