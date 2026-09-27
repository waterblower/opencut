use super::global_settings::GlobalEditorSettings;
use super::*;
use gpui_component::input::{Input, InputState};

impl Editor {
    pub fn open_global_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = GlobalEditorSettings::load();
        self.dismiss_context_menu();
        self.settings_open = false;
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(settings.minimax_api_key)
                .masked(true)
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        self.global_settings_input = Some(input);
        cx.notify();
    }

    pub fn global_settings_dialog(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(input) = self.global_settings_input.as_ref() else {
            return div().into_any_element();
        };
        div().id("global-settings-overlay").absolute().inset_0()
            .flex().items_center().justify_center().occlude().bg(gpui::rgba(0x000000b3))
            .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.stop_propagation()))
            .child(div().id("global-settings-dialog").w(px(460.0)).p_5().flex().flex_col().gap_4()
                .rounded_xl().border_1().border_color(rgb(BORDER)).bg(rgb(PANEL)).shadow_lg()
                .child(div().text_lg().child("Settings"))
                .child(div().text_sm().child("MiniMax API key"))
                .child(Input::new(input))
                .child(div().text_xs().text_color(rgb(MUTED))
                    .child("Saved in global settings for all projects. Used to send audio to MiniMax for SRT generation."))
                .child(div().flex().justify_end().gap_2()
                    .child(settings_button("Cancel").on_click(cx.listener(|editor, _, _, cx| {
                        editor.global_settings_input = None;
                        cx.notify();
                    })))
                    .child(settings_button("Save").on_click(cx.listener(|editor, _, _, cx| {
                        let Some(input) = editor.global_settings_input.as_ref() else { return; };
                        let mut settings = GlobalEditorSettings::load();
                        settings.minimax_api_key = input.read(cx).value().trim().to_string();
                        match settings.save() {
                            Ok(()) => {
                                editor.global_settings_input = None;
                                editor.status = Some("Settings saved.".into());
                            }
                            Err(error) => {
                                log::error!("{error}");
                                editor.status = Some(error.to_string());
                            }
                        }
                        cx.notify();
                    }))))).into_any_element()
    }
}

fn settings_button(label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .px_3()
        .py_2()
        .rounded_md()
        .bg(rgb(SURFACE))
        .cursor(CursorStyle::PointingHand)
        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
        .child(label)
}
