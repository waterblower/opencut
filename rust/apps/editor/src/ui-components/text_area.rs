//! A multiline, wrapping text editor. Enter inserts a newline; Escape returns focus.

use crate::theme::{BORDER, MUTED, SURFACE, TEXT};
use gpui::prelude::*;
use gpui::{
    App, AvailableSpace, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable,
    GlobalElementId, IntoElement, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, Render, SharedString, Size, Style,
    TextRun, UTF16Selection, UnderlineStyle, Window, WrappedLine, actions, div, fill, point, px,
    rgb, rgba, size,
};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

actions!(
    text_area,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        Dismiss,
        Newline
    ]
);

pub(crate) const TEXT_AREA_KEY_CONTEXT: &str = "TextArea";

/// Emitted when the text changes through editing, not through `set_text_silently`.
pub(crate) enum TextAreaEvent {
    Change,
}

impl EventEmitter<TextAreaEvent> for TextArea {}

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(TEXT_AREA_KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("left", Left, context),
        KeyBinding::new("right", Right, context),
        KeyBinding::new("up", Up, context),
        KeyBinding::new("down", Down, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("shift-up", SelectUp, context),
        KeyBinding::new("shift-down", SelectDown, context),
        KeyBinding::new("cmd-a", SelectAll, context),
        KeyBinding::new("home", Home, context),
        KeyBinding::new("end", End, context),
        KeyBinding::new("cmd-left", Home, context),
        KeyBinding::new("cmd-right", End, context),
        KeyBinding::new("cmd-v", Paste, context),
        KeyBinding::new("cmd-x", Cut, context),
        KeyBinding::new("cmd-c", Copy, context),
        KeyBinding::new("escape", Dismiss, context),
        KeyBinding::new("enter", Newline, context),
    ]);
}

pub(crate) struct TextArea {
    focus_handle: FocusHandle,
    return_focus: FocusHandle,
    element_id: SharedString,
    placeholder: SharedString,
    content: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<TextAreaLayout>, // 上一帧的排版，用于鼠标命中、上下移动和输入法定位
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
}

// Public APIs only
impl TextArea {
    pub(crate) fn new(
        element_id: &'static str,
        placeholder: &'static str,
        return_focus: FocusHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle().tab_stop(true),
            return_focus,
            element_id: element_id.into(),
            placeholder: placeholder.into(),
            content: "".into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
        }
    }

    pub(crate) fn text(&self) -> &str {
        self.content.as_ref()
    }

    pub(crate) fn set_text_silently(&mut self, text: String) {
        let cursor = text.len();
        self.content = text.into();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
    }
}

// Private APIs only
impl TextArea {
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
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.content.len())
    }

    /// The offset one visual row above (`rows = -1`) or below (`rows = 1`) the caret,
    /// keeping its x position. Past the first or last row it goes to the start or end.
    fn vertical_offset(&self, rows: f32) -> usize {
        let Some(layout) = self.last_layout.as_ref() else {
            return self.cursor_offset();
        };
        let caret = layout.position_for_offset(self.cursor_offset());
        let target = point(caret.x, caret.y + layout.line_height * (0.5 + rows));
        if target.y < px(0.0) {
            return 0;
        }
        layout.offset_for_point(target, self.content.len())
    }

    /// The start or end of the paragraph that contains the caret.
    fn paragraph_bounds(&self) -> Range<usize> {
        let cursor = self.cursor_offset();
        let start = match self.content[..cursor].rfind('\n') {
            Some(index) => index + 1,
            None => 0,
        };
        let end = match self.content[cursor..].find('\n') {
            Some(index) => cursor + index,
            None => self.content.len(),
        };
        start..end
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(layout)) = (self.last_bounds, self.last_layout.as_ref()) else {
            return 0;
        };
        if self.content.is_empty() {
            return 0;
        }
        let local = point(position.x - bounds.left(), position.y - bounds.top());
        if local.y < px(0.0) {
            return 0;
        }
        layout.offset_for_point(local, self.content.len())
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

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.vertical_offset(-1.0), cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.vertical_offset(1.0), cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.vertical_offset(-1.0), cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.vertical_offset(1.0), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.paragraph_bounds().start, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.paragraph_bounds().end, cx);
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

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_text_in_range(None, "\n", window, cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(
                None,
                &text.replace("\r\n", "\n").replace('\r', "\n"),
                window,
                cx,
            );
        }
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        self.return_focus.focus(window, cx);
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        utf8_offset_from_utf16(&self.content, offset)
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        utf16_offset_from_utf8(&self.content, offset)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }
}

impl EntityInputHandler for TextArea {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
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
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = match range_utf16.as_ref() {
            Some(range) => self.range_from_utf16(range),
            None => match self.marked_range.clone() {
                Some(marked) => marked,
                None => self.selected_range.clone(),
            },
        };
        self.content = format!(
            "{}{}{}",
            &self.content[..range.start],
            new_text,
            &self.content[range.end..]
        )
        .into();
        let cursor = range.start + new_text.len();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
        cx.emit(TextAreaEvent::Change);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = match range_utf16.as_ref() {
            Some(range) => self.range_from_utf16(range),
            None => match self.marked_range.clone() {
                Some(marked) => marked,
                None => self.selected_range.clone(),
            },
        };
        self.content = format!(
            "{}{}{}",
            &self.content[..range.start],
            new_text,
            &self.content[range.end..]
        )
        .into();
        self.marked_range =
            (!new_text.is_empty()).then_some(range.start..range.start + new_text.len());
        self.selected_range = match new_selected_range_utf16.as_ref() {
            Some(selection) => {
                range.start + utf8_offset_from_utf16(new_text, selection.start)
                    ..range.start + utf8_offset_from_utf16(new_text, selection.end)
            }
            None => {
                let cursor = range.start + new_text.len();
                cursor..cursor
            }
        };
        self.selection_reversed = false;
        cx.emit(TextAreaEvent::Change);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let start = layout.position_for_offset(range.start);
        let end = layout.position_for_offset(range.end);
        Some(Bounds::from_corners(
            point(bounds.left() + start.x, bounds.top() + start.y),
            point(
                bounds.left() + end.x,
                bounds.top() + end.y + layout.line_height,
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let layout = self.last_layout.as_ref()?;
        let local = bounds.localize(&point)?;
        let index = layout.offset_for_point(local, self.content.len());
        Some(self.offset_to_utf16(index))
    }
}

impl Render for TextArea {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id(self.element_id.clone())
            .key_context(TEXT_AREA_KEY_CONTEXT)
            .track_focus(&self.focus_handle(cx))
            .w_full()
            .min_h(px(58.0))
            .px(px(10.0))
            .py(px(8.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SURFACE))
            .text_size(px(13.0))
            .line_height(px(19.0))
            .text_color(rgb(TEXT))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::dismiss))
            .on_action(cx.listener(Self::newline))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(TextAreaElement { area: cx.entity() })
    }
}

impl Focusable for TextArea {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

const SELECTION_COLOR: u32 = 0x52779a66;
const CARET_COLOR: u32 = 0xd8d8dc;

/// Wrapped paragraphs of one frame. Paragraphs are separated by `\n` and stacked
/// without gaps, so visual rows are `line_height` apart across paragraph boundaries.
struct TextAreaLayout {
    paragraphs: Vec<ParagraphLayout>,
    line_height: Pixels,
}

struct ParagraphLayout {
    start: usize, // 段落第一个字节在内容中的偏移
    top: Pixels,  // 段落第一行相对文本区顶部的 y
    line: WrappedLine,
}

impl TextAreaLayout {
    /// The top-left corner of the caret slot at `offset`, relative to the text area.
    fn position_for_offset(&self, offset: usize) -> Point<Pixels> {
        for paragraph in &self.paragraphs {
            if offset > paragraph.start + paragraph.line.len() {
                continue;
            }
            let local = match paragraph
                .line
                .position_for_index(offset - paragraph.start, self.line_height)
            {
                Some(local) => local,
                None => point(px(0.0), px(0.0)),
            };
            return point(local.x, paragraph.top + local.y);
        }
        point(px(0.0), px(0.0))
    }

    /// The offset closest to `local`, a point relative to the text area.
    fn offset_for_point(&self, local: Point<Pixels>, content_len: usize) -> usize {
        for paragraph in &self.paragraphs {
            let bottom = paragraph.top + paragraph.line.size(self.line_height).height;
            if local.y >= bottom {
                continue;
            }
            let within = point(local.x, local.y - paragraph.top);
            let index = match paragraph
                .line
                .closest_index_for_position(within, self.line_height)
            {
                Ok(index) => index,
                Err(index) => index,
            };
            return paragraph.start + index;
        }
        content_len
    }

    fn height(&self) -> Pixels {
        match self.paragraphs.last() {
            Some(last) => last.top + last.line.size(self.line_height).height,
            None => self.line_height,
        }
    }
}

struct TextAreaElement {
    area: Entity<TextArea>,
}

struct TextAreaPrepaintState {
    layout: Option<TextAreaLayout>,
    caret: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
}

impl IntoElement for TextAreaElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextAreaElement {
    type RequestLayoutState = ();
    type PrepaintState = TextAreaPrepaintState;

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
        let (text, runs) = shaping_input(self.area.read(cx), window);
        let font_size = window.text_style().font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let mut style = Style::default();
        style.size.width = gpui::relative(1.0).into();
        // 高度取决于换行结果，需在已知宽度后测量。
        let layout_id =
            window.request_measured_layout(style, move |known, available, window, _| {
                let width = match known.width {
                    Some(width) => Some(width),
                    None => match available.width {
                        AvailableSpace::Definite(width) => Some(width),
                        AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
                    },
                };
                let layout = layout_paragraphs(&text, &runs, font_size, line_height, width, window);
                size(width.unwrap_or(px(0.0)), layout.height())
            });
        (layout_id, ())
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
        let area = self.area.read(cx);
        let (text, runs) = shaping_input(area, window);
        let font_size = window.text_style().font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let layout = layout_paragraphs(
            &text,
            &runs,
            font_size,
            line_height,
            Some(bounds.size.width),
            window,
        );
        let selected_range = area.selected_range.clone();
        let caret_offset = area.cursor_offset();
        // 内容为空时显示占位文字，但光标仍在开头。
        let (caret, selection) = if selected_range.is_empty() {
            let position = if area.content.is_empty() {
                point(px(0.0), px(0.0))
            } else {
                layout.position_for_offset(caret_offset)
            };
            let caret = fill(
                Bounds::new(
                    point(bounds.left() + position.x, bounds.top() + position.y),
                    size(px(1.0), line_height),
                ),
                rgb(CARET_COLOR),
            );
            (Some(caret), Vec::new())
        } else {
            let start = layout.position_for_offset(selected_range.start);
            let end = layout.position_for_offset(selected_range.end);
            (None, selection_quads(start, end, line_height, bounds))
        };
        TextAreaPrepaintState {
            layout: Some(layout),
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
        let focus_handle = self.area.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.area.clone()),
            cx,
        );
        for quad in prepaint.selection.drain(..) {
            window.paint_quad(quad);
        }
        let layout = prepaint.layout.take().expect("text area was not laid out");
        for paragraph in &layout.paragraphs {
            let origin = point(bounds.left(), bounds.top() + paragraph.top);
            if let Err(error) = paragraph.line.paint(
                origin,
                layout.line_height,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            ) {
                log::error!("Could not paint text area line: {error:?}");
            }
        }
        if focus_handle.is_focused(window)
            && let Some(caret) = prepaint.caret.take()
        {
            window.paint_quad(caret);
        }
        self.area.update(cx, |area, _| {
            area.last_layout = Some(layout);
            area.last_bounds = Some(bounds);
        });
    }
}

/// The text to shape and its runs: the content with the IME-marked range underlined,
/// or the muted placeholder when the content is empty.
fn shaping_input(area: &TextArea, window: &Window) -> (SharedString, Vec<TextRun>) {
    let style = window.text_style();
    let (text, color) = if area.content.is_empty() {
        (area.placeholder.clone(), rgb(MUTED).into())
    } else {
        (area.content.clone(), style.color)
    };
    let base_run = TextRun {
        len: text.len(),
        font: style.font(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let runs = match area.marked_range.as_ref() {
        Some(marked) if !area.content.is_empty() => vec![
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
                len: text.len() - marked.end,
                ..base_run
            },
        ],
        Some(_) | None => vec![base_run],
    };
    (text, runs)
}

fn layout_paragraphs(
    text: &SharedString,
    runs: &[TextRun],
    font_size: Pixels,
    line_height: Pixels,
    wrap_width: Option<Pixels>,
    window: &Window,
) -> TextAreaLayout {
    let lines =
        match window
            .text_system()
            .shape_text(text.clone(), font_size, runs, wrap_width, None)
        {
            Ok(lines) => lines,
            Err(error) => {
                log::error!("Could not shape text area content: {error:?}");
                Default::default()
            }
        };
    let mut paragraphs = Vec::with_capacity(lines.len());
    let mut start = 0;
    let mut top = px(0.0);
    for line in lines {
        let height = line.size(line_height).height;
        let len = line.len();
        paragraphs.push(ParagraphLayout { start, top, line });
        start += len + '\n'.len_utf8();
        top += height;
    }
    TextAreaLayout {
        paragraphs,
        line_height,
    }
}

/// One rectangle per visual row between two caret positions (relative to the text area).
fn selection_quads(
    start: Point<Pixels>,
    end: Point<Pixels>,
    line_height: Pixels,
    bounds: Bounds<Pixels>,
) -> Vec<PaintQuad> {
    let quad = |left: Pixels, top: Pixels, right: Pixels, bottom: Pixels| {
        fill(
            Bounds::from_corners(
                point(bounds.left() + left, bounds.top() + top),
                point(bounds.left() + right, bounds.top() + bottom),
            ),
            rgba(SELECTION_COLOR),
        )
    };
    if start.y == end.y {
        return vec![quad(start.x, start.y, end.x, start.y + line_height)];
    }
    let width = bounds.size.width;
    let mut quads = vec![quad(start.x, start.y, width, start.y + line_height)];
    if end.y > start.y + line_height {
        quads.push(quad(px(0.0), start.y + line_height, width, end.y));
    }
    quads.push(quad(px(0.0), end.y, end.x, end.y + line_height));
    quads
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
