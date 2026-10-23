//! A single-line text editor. Escape returns focus to the caller.
//!
//! The caller owns the text: it draws `TextInputV2::new(id, text, return_focus)` every frame and
//! receives each edit through `on_text_change`. Caret, selection, IME and focus are kept in gpui's
//! per-element state under `id`, so they survive between frames without the caller.

use crate::theme::ACCENT;
use gpui::prelude::*;
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, GlobalElementId, IntoElement, KeyBinding, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    ShapedLine, SharedString, Style, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div,
    fill, point, px, relative, rgb, rgba, size,
};
use std::ops::Range;
use std::rc::Rc;
use unicode_segmentation::UnicodeSegmentation;

actions!(
    text_input_v2,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        Dismiss
    ]
);

const TEXT_INPUT_V2_KEY_CONTEXT: &str = "TextInputV2";

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(TEXT_INPUT_V2_KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("left", Left, context),
        KeyBinding::new("right", Right, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("cmd-a", SelectAll, context),
        KeyBinding::new("home", Home, context),
        KeyBinding::new("end", End, context),
        KeyBinding::new("cmd-left", Home, context),
        KeyBinding::new("cmd-right", End, context),
        KeyBinding::new("cmd-v", Paste, context),
        KeyBinding::new("cmd-x", Cut, context),
        KeyBinding::new("cmd-c", Copy, context),
        KeyBinding::new("escape", Dismiss, context),
    ]);
}

/// Which characters typing and pasting may insert.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum InputConstraint {
    Any,
    Number,
}

/// Draws an editor for `text`, which the caller owns.
#[derive(IntoElement)]
pub(crate) struct TextInputV2 {
    id: ElementId,
    text: SharedString,
    return_focus: FocusHandle,
    constraint: InputConstraint,
    on_text_change: Option<TextCallback>,
}

// Public APIs only
impl TextInputV2 {
    /// `id` keys the caret, selection and focus kept between frames; it must be unique
    /// among its siblings.
    pub(crate) fn new(
        id: impl Into<ElementId>,
        text: impl Into<SharedString>,
        return_focus: FocusHandle,
    ) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            return_focus,
            constraint: InputConstraint::Any,
            on_text_change: None,
        }
    }

    pub(crate) fn constraint(mut self, constraint: InputConstraint) -> Self {
        self.constraint = constraint;
        self
    }

    /// Called with the full text after every edit.
    pub(crate) fn on_text_change(
        mut self,
        handler: impl Fn(&String, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_text_change = Some(Rc::new(handler));
        self
    }
}

type TextCallback = Rc<dyn Fn(&String, &mut Window, &mut App)>;

const SELECTION_COLOR: u32 = 0x52779a66;

struct TextInputState {
    focus_handle: FocusHandle,
    return_focus: FocusHandle,
    text: SharedString,                    // 包含尚未由调用方确认的输入草稿
    last_drawn_text: Option<SharedString>, // 上次收到的模型快照，用于识别撤销等外部修改
    constraint: InputConstraint,           // 每帧由 TextInputV2 重新设置
    on_text_change: Option<TextCallback>,  // 每帧由 TextInputV2 重新设置
    selected_range: Range<usize>,          // text 中的字节偏移，不是显示文本中的偏移
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_line: Option<ShapedLine>, // 上一帧排版的显示文本，用于鼠标命中和输入法定位
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
}

// Private APIs only
impl TextInputState {
    fn new(text: SharedString, return_focus: FocusHandle, cx: &mut Context<Self>) -> Self {
        let cursor = text.len();
        Self {
            focus_handle: cx.focus_handle().tab_stop(true),
            return_focus,
            text: text.clone(),
            last_drawn_text: Some(text),
            constraint: InputConstraint::Any,
            on_text_change: None,
            selected_range: cursor..cursor,
            selection_reversed: false,
            marked_range: None,
            last_line: None,
            last_bounds: None,
            is_selecting: false,
        }
    }

    /// Takes the text the caller is drawing. When it differs from the current one (an edit
    /// elsewhere, or a rejected edit), the selection is clamped to the new text.
    fn set_drawn_text(&mut self, text: SharedString) {
        if self.text == text {
            return;
        }
        self.selected_range = char_boundary_at_or_before(&text, self.selected_range.start)
            ..char_boundary_at_or_before(&text, self.selected_range.end);
        self.marked_range = None;
        self.text = text;
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.text.len())
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (self.last_bounds, self.last_line.as_ref()) else {
            return 0;
        };
        if self.text.is_empty() {
            return 0;
        }
        if position.x < bounds.left() {
            return 0;
        }
        if position.x > bounds.right() {
            return self.text.len();
        }
        line.closest_index_for_x(position.x - bounds.left())
    }

    fn filtered_input(&self, text: &str) -> String {
        match self.constraint {
            InputConstraint::Any => text.replace(['\r', '\n'], " "),
            InputConstraint::Number => text
                .chars()
                .filter(|character| character.is_ascii_digit() || matches!(character, '.' | '-'))
                .collect(),
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window, cx);
        self.is_selecting = true;
        let offset = self.index_for_mouse_position(event.position);
        if event.modifiers.shift {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.text.len(), cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.text.len(), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.text[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.text[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        self.return_focus.focus(window, cx);
    }

    fn notify_text_change(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(on_text_change) = self.on_text_change.clone() {
            on_text_change(&self.text.to_string(), window, cx);
        }
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        utf8_offset_from_utf16(&self.text, offset)
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        utf16_offset_from_utf8(&self.text, offset)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }
}

impl EntityInputHandler for TextInputState {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.text[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = match range_utf16.as_ref() {
            Some(range) => self.range_from_utf16(range),
            None => match self.marked_range.clone() {
                Some(marked) => marked,
                None => self.selected_range.clone(),
            },
        };
        let inserted_text = self.filtered_input(new_text);
        self.text = format!(
            "{}{}{}",
            &self.text[..range.start],
            inserted_text,
            &self.text[range.end..]
        )
        .into();
        let cursor = range.start + inserted_text.len();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
        self.notify_text_change(window, cx);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = match range_utf16.as_ref() {
            Some(range) => self.range_from_utf16(range),
            None => match self.marked_range.clone() {
                Some(marked) => marked,
                None => self.selected_range.clone(),
            },
        };
        let inserted_text = self.filtered_input(new_text);
        self.text = format!(
            "{}{}{}",
            &self.text[..range.start],
            inserted_text,
            &self.text[range.end..]
        )
        .into();
        self.marked_range =
            (!inserted_text.is_empty()).then_some(range.start..range.start + inserted_text.len());
        // 过滤改变了文本时，输入法给出的选区不再对应插入的文本，改为放在末尾。
        self.selected_range = match new_selected_range_utf16.as_ref() {
            Some(selection) if inserted_text == new_text => {
                range.start + utf8_offset_from_utf16(new_text, selection.start)
                    ..range.start + utf8_offset_from_utf16(new_text, selection.end)
            }
            Some(_) | None => {
                let cursor = range.start + inserted_text.len();
                cursor..cursor
            }
        };
        self.selection_reversed = false;
        self.notify_text_change(window, cx);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.last_line.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(bounds.left() + line.x_for_index(range.start), bounds.top()),
            point(bounds.left() + line.x_for_index(range.end), bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let line = self.last_line.as_ref()?;
        let local = bounds.localize(&point)?;
        let index = line.closest_index_for_x(local.x);
        Some(self.offset_to_utf16(index))
    }
}

impl RenderOnce for TextInputV2 {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| {
            TextInputState::new(self.text.clone(), self.return_focus.clone(), cx)
        });
        let focus_handle = state.read(cx).focus_handle.clone();
        div()
            .id(self.id)
            .key_context(TEXT_INPUT_V2_KEY_CONTEXT)
            .track_focus(&focus_handle)
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .cursor(CursorStyle::IBeam)
            .on_action(window.listener_for(&state, TextInputState::backspace))
            .on_action(window.listener_for(&state, TextInputState::delete))
            .on_action(window.listener_for(&state, TextInputState::left))
            .on_action(window.listener_for(&state, TextInputState::right))
            .on_action(window.listener_for(&state, TextInputState::select_left))
            .on_action(window.listener_for(&state, TextInputState::select_right))
            .on_action(window.listener_for(&state, TextInputState::select_all))
            .on_action(window.listener_for(&state, TextInputState::home))
            .on_action(window.listener_for(&state, TextInputState::end))
            .on_action(window.listener_for(&state, TextInputState::paste))
            .on_action(window.listener_for(&state, TextInputState::cut))
            .on_action(window.listener_for(&state, TextInputState::copy))
            .on_action(window.listener_for(&state, TextInputState::dismiss))
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&state, TextInputState::on_mouse_down),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&state, TextInputState::on_mouse_up),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                window.listener_for(&state, TextInputState::on_mouse_up),
            )
            .on_mouse_move(window.listener_for(&state, TextInputState::on_mouse_move))
            .child(TextInputElement {
                state,
                text: self.text,
                constraint: self.constraint,
                on_text_change: self.on_text_change,
            })
    }
}

struct TextInputElement {
    state: Entity<TextInputState>,
    text: SharedString,
    constraint: InputConstraint,
    on_text_change: Option<TextCallback>,
}

struct TextInputPrepaintState {
    line: Option<ShapedLine>,
    caret: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for TextInputElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextInputElement {
    type RequestLayoutState = ();
    type PrepaintState = TextInputPrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        // 相同的旧快照不能覆盖输入草稿；模型变化（包括撤销/重做）即使聚焦也要同步。
        let focused = self.state.read(cx).focus_handle.is_focused(window);
        let drawn_text = self.text.clone();
        let constraint = self.constraint;
        let on_text_change = self.on_text_change.clone();
        self.state.update(cx, |state, _| {
            state.constraint = constraint;
            state.on_text_change = on_text_change;
            // 聚焦时，数字草稿与模型值相等（如 "1." 对 "1.00"）就保留草稿，不打断输入。
            let draft_is_same_number = focused
                && constraint == InputConstraint::Number
                && matches!(
                    (state.text.parse::<f64>(), drawn_text.parse::<f64>()),
                    (Ok(draft), Ok(drawn)) if draft == drawn
                );
            if !draft_is_same_number
                && (!focused || state.last_drawn_text.as_ref() != Some(&drawn_text))
            {
                state.set_drawn_text(drawn_text.clone());
            }
            state.last_drawn_text = Some(drawn_text);
        });
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.state.read(cx);
        let selected_range = input.selected_range.clone();
        let caret_offset = input.cursor_offset();
        let style = window.text_style();
        let display_text = input.text.clone();
        let base_run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: style.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = match input.marked_range.as_ref() {
            Some(marked) => [
                TextRun {
                    len: marked.start,
                    ..base_run.clone()
                },
                TextRun {
                    len: marked.end - marked.start,
                    underline: Some(UnderlineStyle {
                        color: Some(base_run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..base_run.clone()
                },
                TextRun {
                    len: display_text.len() - marked.end,
                    ..base_run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect(),
            None => vec![base_run],
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);
        let caret_x = line.x_for_index(caret_offset);
        let (caret, selection) = if selected_range.is_empty() {
            let caret = fill(
                Bounds::new(
                    point(bounds.left() + caret_x, bounds.top()),
                    size(px(1.5), bounds.size.height),
                ),
                rgb(ACCENT),
            );
            (Some(caret), None)
        } else {
            let selection = fill(
                Bounds::from_corners(
                    point(
                        bounds.left() + line.x_for_index(selected_range.start),
                        bounds.top(),
                    ),
                    point(
                        bounds.left() + line.x_for_index(selected_range.end),
                        bounds.bottom(),
                    ),
                ),
                rgba(SELECTION_COLOR),
            );
            (None, Some(selection))
        };
        TextInputPrepaintState {
            line: Some(line),
            caret,
            selection,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.state.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.state.clone()),
            cx,
        );
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection);
        }
        let line = prepaint.line.take().expect("text input was not shaped");
        if let Err(error) = line.paint(
            bounds.origin,
            window.line_height(),
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        ) {
            log::error!("Could not paint text input line: {error:?}");
        }
        if focus_handle.is_focused(window)
            && let Some(caret) = prepaint.caret.take()
        {
            window.paint_quad(caret);
        }
        self.state.update(cx, |input, _| {
            input.last_line = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

/// The largest char boundary in `text` that is not after `offset`.
fn char_boundary_at_or_before(text: &str, offset: usize) -> usize {
    let mut boundary = offset.min(text.len());
    while !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

fn utf8_offset_from_utf16(text: &str, offset: usize) -> usize {
    let mut utf8_offset = 0;
    let mut utf16_count = 0;
    for character in text.chars() {
        if utf16_count >= offset {
            break;
        }
        utf16_count += character.len_utf16();
        utf8_offset += character.len_utf8();
    }
    utf8_offset
}

fn utf16_offset_from_utf8(text: &str, offset: usize) -> usize {
    let mut utf16_offset = 0;
    let mut utf8_count = 0;
    for character in text.chars() {
        if utf8_count >= offset {
            break;
        }
        utf8_count += character.len_utf8();
        utf16_offset += character.len_utf16();
    }
    utf16_offset
}
