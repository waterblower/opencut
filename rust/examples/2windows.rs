use gpui::{
    App, Bounds, Hsla, Render, Window, WindowBounds, WindowOptions, div, hsla, prelude::*, px, size,
};
use gpui_platform::application;

fn main() {
    env_logger::init();

    application().run(move |cx: &mut App| {
        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1100.0), px(760.0)), cx);
        let example = cx.new(|_| Example { n: 0 });
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..WindowOptions::default()
            },
            move |_, _cx| example,
        )
        .expect("failed to create the GPUI window");
        cx.activate(true);
    });
}

#[derive(Clone, Copy)]
struct Example {
    n: u32,
}

const BLACK: Hsla = hsla(0.0, 0.0, 0.0, 1.0);

impl Render for Example {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Example>) -> impl IntoElement {
        let example = cx.entity();
        let new_window_button = button("New window").on_click(move |_event, _window, cx| {
            let bounds = Bounds::centered(None, size(px(1100.0), px(760.0)), cx);
            let example = example.clone();
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    focus: true,
                    ..WindowOptions::default()
                },
                move |_, _| example,
            )
            .expect("failed to create window");
        });
        let add_button = button("Add").on_click(cx.listener(|example, _event, _win, cx| {
            example.n += 1;
            cx.notify();
        }));

        div()
            .child(format!("n: {}", self.n))
            .bg(hsla(1.0, 1.0, 1.0, 1.0))
            .child(new_window_button)
            .child(add_button)
    }
}

fn button(text: &'static str) -> gpui::Stateful<gpui::Div> {
    let button = div()
        .id(text)
        .h_10()
        .px_4()
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .border_1()
        .border_color(hsla(0.0, 0.0, 0.22, 1.0))
        .bg(BLACK)
        .text_sm()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(hsla(0.0, 0.0, 1.0, 1.0))
        .shadow_sm()
        .cursor_pointer()
        .hover(|style| {
            style
                .bg(hsla(0.0, 0.0, 0.18, 1.0))
                .border_color(hsla(0.0, 0.0, 0.32, 1.0))
        })
        .active(|style| style.bg(hsla(0.0, 0.0, 0.08, 1.0)).shadow_none())
        .child(text);
    return button;
}
