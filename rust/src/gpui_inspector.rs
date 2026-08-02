use gpui::{
    AnyElement, App, Context, CursorStyle, DivInspectorState, Inspector, InspectorElementId,
    MouseButton, Window, div, prelude::*, px, rgb,
};
use std::{
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const PANEL: u32 = 0x111114;
const SURFACE: u32 = 0x18181c;
const SURFACE_HOVER: u32 = 0x222228;
const BORDER: u32 = 0x303036;
const TEXT: u32 = 0xf0f0f2;
const MUTED: u32 = 0x777780;
const ACCENT: u32 = 0xf0b75e;
// GPUI reserves this width for its docked inspector in `Window::draw_roots`.
const INSPECTOR_WIDTH_REMS: f32 = 30.0;

static INSPECTOR_OPEN: AtomicBool = AtomicBool::new(false);
static RENDER_FPS: OnceLock<Mutex<RenderFps>> = OnceLock::new();

struct RenderFps {
    frames: u32,
    value: f32,
    sample_started: Instant,
}

impl RenderFps {
    fn new() -> Self {
        Self {
            frames: 0,
            value: 0.0,
            sample_started: Instant::now(),
        }
    }
}

pub(crate) fn init(cx: &mut App) {
    cx.register_inspector_element(|_: InspectorElementId, state: &DivInspectorState, _, _| {
        render_div_state(state)
    });
    cx.set_inspector_renderer(Box::new(render_inspector));
}

pub(crate) fn toggle(window: &mut Window, cx: &mut App) {
    let was_open = INSPECTOR_OPEN.fetch_xor(true, Ordering::Relaxed);
    if !was_open {
        reset_render_fps();
    }
    window.toggle_inspector(cx);
}

pub(crate) fn close(window: &mut Window, cx: &mut App) {
    INSPECTOR_OPEN.store(false, Ordering::Relaxed);
    window.toggle_inspector(cx);
}

pub(crate) fn docked_width(window: &Window) -> f32 {
    if INSPECTOR_OPEN.load(Ordering::Relaxed) {
        f32::from(window.rem_size()) * INSPECTOR_WIDTH_REMS
    } else {
        0.0
    }
}

fn render_inspector(
    inspector: &mut Inspector,
    window: &mut Window,
    cx: &mut Context<Inspector>,
) -> AnyElement {
    INSPECTOR_OPEN.store(true, Ordering::Relaxed);
    let render_fps = record_render_frame();
    let selected = inspector.active_element_id().cloned();
    let states = inspector.render_inspector_states(window, cx);

    div()
        .id("opencut-gpui-inspector")
        .size_full()
        .flex()
        .flex_col()
        .border_l_1()
        .border_color(rgb(BORDER))
        .bg(rgb(PANEL))
        .text_color(rgb(TEXT))
        .child(
            div()
                .h(px(54.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .border_b_1()
                .border_color(rgb(BORDER))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("gpui-inspector-pick")
                                .h_8()
                                .px_3()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor(CursorStyle::PointingHand)
                                .rounded_md()
                                .border_1()
                                .border_color(if inspector.is_picking() {
                                    rgb(ACCENT)
                                } else {
                                    rgb(BORDER)
                                })
                                .bg(if inspector.is_picking() {
                                    rgb(0x2b2419)
                                } else {
                                    rgb(SURFACE)
                                })
                                .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                .text_xs()
                                .child(if inspector.is_picking() {
                                    "Picking…"
                                } else {
                                    "Pick element"
                                })
                                .on_click(cx.listener(|inspector, _, window, cx| {
                                    inspector.start_picking();
                                    window.refresh();
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("GPUI Inspector"),
                        ),
                )
                .child(
                    div()
                        .id("gpui-inspector-close")
                        .size_8()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor(CursorStyle::PointingHand)
                        .rounded_md()
                        .text_color(rgb(MUTED))
                        .hover(|style| style.bg(rgb(SURFACE_HOVER)).text_color(rgb(TEXT)))
                        .child("×")
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(|_, window, cx| {
                            window.defer(cx, close);
                        }),
                ),
        )
        .child(
            div()
                .id("gpui-inspector-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .child(render_fps_section(render_fps))
                .when_some(selected, |this, id| this.child(render_element_id(&id)))
                .when(states.is_empty(), |this| {
                    this.child(
                        div()
                            .rounded_lg()
                            .border_1()
                            .border_color(rgb(BORDER))
                            .bg(rgb(SURFACE))
                            .p_3()
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child(if inspector.is_picking() {
                                "Move over an element and click to inspect it. Scroll while picking to select an occluded layer."
                            } else {
                                "Select Pick element, then click an element in the editor."
                            }),
                    )
                })
                .children(states),
        )
        .into_any_element()
}

fn record_render_frame() -> f32 {
    let mut fps = RENDER_FPS
        .get_or_init(|| Mutex::new(RenderFps::new()))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    fps.frames = fps.frames.saturating_add(1);
    let elapsed = fps.sample_started.elapsed();
    if elapsed >= Duration::from_secs(1) {
        fps.value = fps.frames as f32 / elapsed.as_secs_f32();
        fps.frames = 0;
        fps.sample_started = Instant::now();
    }
    fps.value
}

fn reset_render_fps() {
    let mut fps = RENDER_FPS
        .get_or_init(|| Mutex::new(RenderFps::new()))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    *fps = RenderFps::new();
}

fn render_fps_section(render_fps: f32) -> gpui::Div {
    section("RENDERING")
        .child(
            div()
                .h(px(64.0))
                .flex()
                .items_center()
                .justify_between()
                .rounded_lg()
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(0x0d0d10))
                .px_4()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().size_2().rounded_full().bg(rgb(0x63d68b)))
                        .child("Render FPS"),
                )
                .child(
                    div()
                        .font_family("monospace")
                        .text_lg()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(ACCENT))
                        .child(format!("{render_fps:.1}")),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(MUTED))
                .child("GPUI render passes per second"),
        )
}

fn render_element_id(id: &InspectorElementId) -> AnyElement {
    let location = id.path.source_location;
    section("ELEMENT")
        .child(property("ID", id.path.global_id.to_string()))
        .child(property("Instance", id.instance_id.to_string()))
        .child(property(
            "Source",
            format!(
                "{}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            ),
        ))
        .into_any_element()
}

fn render_div_state(state: &DivInspectorState) -> AnyElement {
    let bounds = state.bounds;
    let content = state.content_size;
    section("DIV")
        .child(property(
            "Position",
            format!(
                "x {:.1} · y {:.1}",
                f32::from(bounds.origin.x),
                f32::from(bounds.origin.y)
            ),
        ))
        .child(property(
            "Size",
            format!(
                "{:.1} × {:.1}",
                f32::from(bounds.size.width),
                f32::from(bounds.size.height)
            ),
        ))
        .child(property(
            "Content",
            format!(
                "{:.1} × {:.1}",
                f32::from(content.width),
                f32::from(content.height)
            ),
        ))
        .child(div().mt_2().text_xs().text_color(rgb(MUTED)).child("STYLE"))
        .child(
            div()
                .rounded_md()
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(0x0d0d10))
                .p_2()
                .font_family("monospace")
                .text_xs()
                .text_color(rgb(0xa8a8b0))
                .child(format!("{:#?}", state.base_style)),
        )
        .into_any_element()
}

fn section(title: &'static str) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .rounded_lg()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE))
        .p_3()
        .child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(ACCENT))
                .child(title),
        )
}

fn property(label: &'static str, value: String) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().text_color(rgb(MUTED)).child(label))
        .child(
            div()
                .font_family("monospace")
                .text_xs()
                .text_color(rgb(TEXT))
                .child(value),
        )
}
