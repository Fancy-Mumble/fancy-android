//! A text field that takes input through GPUI's input handler, as the
//! client's fields do: Zed's `crates/gpui/examples/input.rs` (Apache-2.0),
//! without its desktop entry point, grown into a multi-line composer.
//!
//! It holds several lines (split at line breaks), places the caret where a
//! tap lands, pastes the system clipboard on a long press (the platform's
//! secondary click), and has two modes: a Send key on the keyboard (enter
//! sends), or a line-break key with the app's own Send button.
#![allow(dead_code)]

use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, Keystroke, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    ShapedLine, SharedString, Style, TextRun, UTF16Selection, UnderlineStyle, Window, actions,
    black, div, fill, hsla, point, prelude::*, px, relative, rgb, rgba, size, white,
};
use unicode_segmentation::*;

actions!(
    text_input,
    [
        Backspace, Delete, Left, Right, SelectLeft, SelectRight, SelectAll, Home, End, Paste, Cut,
        Copy, Send,
    ]
);

/// One laid-out line of the field: where it starts in the text, and its shape.
struct LaidLine {
    start: usize,
    line: ShapedLine,
}

struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Vec<LaidLine>,
    last_bounds: Option<Bounds<Pixels>>,
    line_height: Pixels,
    is_selecting: bool,
    /// Line-break mode: the keyboard's enter key types a line break and the
    /// app's own button sends; otherwise the keyboard shows Send.
    multi_line: bool,
}

impl TextInput {
    fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), window, cx);
        } else {
            self.move_to(self.selected_range.start, window, cx)
        }
    }

    fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), window, cx);
        } else {
            self.move_to(self.selected_range.end, window, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), window, cx);
    }

    fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), window, cx);
    }

    fn select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, window, cx);
        self.select_to(self.content.len(), window, cx)
    }

    fn home(&mut self, _: &Home, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, window, cx);
    }

    fn end(&mut self, _: &End, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), window, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            if self.cursor_offset() == prev {
                return;
            }
            self.select_to(prev, window, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if self.cursor_offset() == next {
                return;
            }
            self.select_to(next, window, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        window.request_virtual_keyboard();
        let index = self.index_for_mouse_position(event.position);
        log::info!("fancy-hello caret placed at {index} by a tap");
        if event.modifiers.shift {
            self.select_to(index, window, cx);
        } else {
            self.move_to(index, window, cx)
        }
    }

    /// A long press (the secondary click) pastes, where the caret is.
    fn on_secondary_down(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.paste(&Paste, window, cx);
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), window, cx);
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        let text = cx.read_from_clipboard().and_then(|item| item.text());
        log::info!("fancy-hello paste: {text:?}");
        if let Some(text) = text {
            self.replace_text_in_range(None, &text, window, cx);
        }
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
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    /// The keyboard's Send key (or enter): logs the text and empties the field.
    fn send(&mut self, _: &Send, window: &mut Window, cx: &mut Context<Self>) {
        log::info!("fancy-hello sent: {:?}", self.content);
        self.reset(window, cx);
    }

    fn move_to(&mut self, offset: usize, window: &Window, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    /// The laid-out line holding byte `offset`.
    fn line_at(&self, offset: usize) -> Option<&LaidLine> {
        self.last_layout
            .iter()
            .rev()
            .find(|laid| laid.start <= offset)
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let Some(bounds) = self.last_bounds else {
            return 0;
        };
        if self.content.is_empty() || self.last_layout.is_empty() {
            return 0;
        }
        let row = ((position.y - bounds.top()) / self.line_height).floor().max(0.) as usize;
        let laid = &self.last_layout[row.min(self.last_layout.len() - 1)];
        laid.start + laid.line.closest_index_for_x(position.x - bounds.left())
    }

    fn select_to(&mut self, offset: usize, window: &Window, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    fn reset(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.content = "".into();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        self.last_layout.clear();
        self.is_selecting = false;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    /// The point where byte `offset` sits, relative to the field's origin.
    fn point_for(&self, offset: usize) -> Point<Pixels> {
        let row = self
            .last_layout
            .iter()
            .rposition(|laid| laid.start <= offset)
            .unwrap_or(0);
        let x = self
            .last_layout
            .get(row)
            .map_or(px(0.), |laid| laid.line.x_for_index(offset - laid.start));
        point(x, self.line_height * row as f32)
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_range = self.range_from_utf16(&range_utf16);
        self.selection_reversed = false;
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.content.encode_utf16().count())
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.marked_range = None;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.selection_reversed = false;
        self.marked_range.take();
        window.notify_text_input_changed(&self.focus_handle);
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
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.marked_range =
            (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        let insertion_start_utf16 = self.offset_to_utf16(range.start);
        let inserted_length_utf16 = new_text.encode_utf16().count();
        self.selected_range = new_selected_range_utf16
            .map(|selected| {
                self.range_from_utf16(
                    &(insertion_start_utf16 + selected.start.min(inserted_length_utf16)
                        ..insertion_start_utf16 + selected.end.min(inserted_length_utf16)),
                )
            })
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        self.selection_reversed = false;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let start = self.point_for(range.start);
        let end = self.point_for(range.end);
        Some(Bounds::from_corners(
            bounds.origin + start,
            bounds.origin + point(end.x, end.y + self.line_height),
        ))
    }

    fn selection_bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Bounds<Pixels>> {
        self.bounds_for_range(range_utf16, bounds, window, cx)
            .into_iter()
            .collect()
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        self.last_bounds?;
        Some(self.offset_to_utf16(self.index_for_mouse_position(point)))
    }

    fn text_input_configuration(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> gpui::TextInputConfiguration {
        // A messaging composer: suggestions, autocorrection, sentence
        // capitals, and a Send key unless line breaks are wanted.
        gpui::TextInputConfiguration {
            autocorrect: true,
            autocapitalize: gpui::Autocapitalize::Sentences,
            suggestions: true,
            input_action: if self.multi_line {
                gpui::TextInputAction::Enter
            } else {
                gpui::TextInputAction::Send
            },
        }
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    lines: Vec<LaidLine>,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// The runs of `line` (bytes `start..` of the text): one colour, the
/// composing part underlined.
fn runs_for(
    start: usize,
    len: usize,
    base: &TextRun,
    marked: Option<&Range<usize>>,
) -> Vec<TextRun> {
    let end = start + len;
    let Some(marked) = marked.filter(|m| m.start < end && m.end > start) else {
        return vec![TextRun { len, ..base.clone() }];
    };
    let from = marked.start.max(start) - start;
    let to = marked.end.min(end) - start;
    [
        TextRun { len: from, ..base.clone() },
        TextRun {
            len: to - from,
            underline: Some(UnderlineStyle {
                color: Some(base.color),
                thickness: px(1.0),
                wavy: false,
            }),
            ..base.clone()
        },
        TextRun { len: len - to, ..base.clone() },
    ]
    .into_iter()
    .filter(|run| run.len > 0)
    .collect()
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let lines = self.input.read(cx).content.split('\n').count().max(1);
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = (window.line_height() * lines as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let marked = input.marked_range.clone();
        let style = window.text_style();
        let line_height = window.line_height();
        let font_size = style.font_size.to_pixels(window.rem_size());

        let (display_text, text_color): (SharedString, _) = if content.is_empty() {
            (input.placeholder.clone(), hsla(0., 0., 0., 0.35))
        } else {
            (content.clone(), rgb(0x101018).into())
        };
        let base = TextRun {
            len: 0,
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
            letter_spacing: Default::default(),
        };

        let mut lines = Vec::new();
        let mut start = 0;
        for text in display_text.split('\n') {
            let runs = runs_for(start, text.len(), &base, marked.as_ref());
            let line = window.text_system().shape_line(
                SharedString::from(text.to_string()),
                font_size,
                &runs,
                None,
            );
            lines.push(LaidLine { start, line });
            start += text.len() + 1;
        }

        let row_of = |offset: usize| lines.iter().rposition(|l| l.start <= offset).unwrap_or(0);
        let at = |offset: usize| {
            let row = row_of(offset);
            let laid = &lines[row];
            point(
                bounds.left() + laid.line.x_for_index(offset - laid.start),
                bounds.top() + line_height * row as f32,
            )
        };
        let (selection, cursor) = if selected_range.is_empty() || content.is_empty() {
            let origin = if content.is_empty() { bounds.origin } else { at(cursor) };
            (
                Vec::new(),
                Some(fill(Bounds::new(origin, size(px(2.), line_height)), gpui::blue())),
            )
        } else {
            let (from, to) = (row_of(selected_range.start), row_of(selected_range.end));
            let selection = (from..=to)
                .map(|row| {
                    let laid = &lines[row];
                    let left = if row == from {
                        at(selected_range.start).x
                    } else {
                        bounds.left()
                    };
                    let right = if row == to {
                        at(selected_range.end).x
                    } else {
                        bounds.left() + laid.line.width
                    };
                    let top = bounds.top() + line_height * row as f32;
                    fill(
                        Bounds::from_corners(point(left, top), point(right, top + line_height)),
                        rgba(0x3311ff30),
                    )
                })
                .collect();
            (selection, None)
        };
        PrepaintState {
            lines,
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        for selection in prepaint.selection.drain(..) {
            window.paint_quad(selection)
        }
        let line_height = window.line_height();
        for (row, laid) in prepaint.lines.iter().enumerate() {
            let origin = point(bounds.left(), bounds.top() + line_height * row as f32);
            laid.line
                .paint(origin, line_height, gpui::TextAlign::Left, None, window, cx)
                .unwrap();
        }
        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }
        let lines = std::mem::take(&mut prepaint.lines);
        let empty = self.input.read(cx).content.is_empty();
        self.input.update(cx, |input, _cx| {
            input.last_layout = if empty { Vec::new() } else { lines };
            input.last_bounds = Some(bounds);
            input.line_height = line_height;
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .key_context("TextInput")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::send))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_secondary_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .line_height(px(30.))
            .text_size(px(22.))
            .child(
                div()
                    .w_full()
                    .p(px(4.))
                    .bg(white())
                    .child(TextElement { input: cx.entity() }),
            )
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// The composer: the field, the mode switch and a Send button, and the
/// last keystrokes the platform sent.
pub struct InputExample {
    text_input: Entity<TextInput>,
    recent_keystrokes: Vec<Keystroke>,
    focus_handle: FocusHandle,
}

impl Focusable for InputExample {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for InputExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let multi_line = self.text_input.read(cx).multi_line;
        let field = self.text_input.clone();
        let mode = self.text_input.clone();
        let keys = self
            .recent_keystrokes
            .iter()
            .rev()
            .map(|ks| ks.unparse(cx.platform_info().os))
            .collect::<Vec<_>>()
            .join("  ");
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .text_size(px(14.))
                    .child(
                        div()
                            .id("mode")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(0x414868))
                            .child(if multi_line {
                                "Mode: line breaks"
                            } else {
                                "Mode: Send key"
                            })
                            .on_click(move |_, window, cx| {
                                mode.update(cx, |input, cx| {
                                    input.multi_line = !input.multi_line;
                                    log::info!("fancy-hello multi-line {}", input.multi_line);
                                    window.notify_text_input_changed(&input.focus_handle);
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        div()
                            .id("send")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(0x7aa2f7))
                            .text_color(black())
                            .child("Send")
                            .on_click(move |_, window, cx| {
                                field.update(cx, |input, cx| input.send(&Send, window, cx));
                            }),
                    )
                    .child(div().flex_1().overflow_hidden().child(keys)),
            )
            .child(self.text_input.clone())
    }
}

/// Binds the field's keys and makes the view holding it.
pub fn new_example(window: &mut Window, cx: &mut App) -> Entity<InputExample> {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, None),
        KeyBinding::new("delete", Delete, None),
        KeyBinding::new("left", Left, None),
        KeyBinding::new("right", Right, None),
        KeyBinding::new("shift-left", SelectLeft, None),
        KeyBinding::new("shift-right", SelectRight, None),
        KeyBinding::new("home", Home, None),
        KeyBinding::new("end", End, None),
        KeyBinding::new("enter", Send, Some("TextInput")),
    ]);
    let text_input = cx.new(|cx| TextInput {
        focus_handle: cx.focus_handle(),
        content: "".into(),
        placeholder: "Type here...".into(),
        selected_range: 0..0,
        selection_reversed: false,
        marked_range: None,
        last_layout: Vec::new(),
        last_bounds: None,
        line_height: px(30.),
        is_selecting: false,
        multi_line: false,
    });
    let _ = window;
    cx.new(|cx| InputExample {
        text_input,
        recent_keystrokes: vec![],
        focus_handle: cx.focus_handle(),
    })
}

impl InputExample {
    /// Notes a keystroke, to show what the platform sent.
    pub fn note(&mut self, keystroke: Keystroke) {
        self.recent_keystrokes.push(keystroke);
        if self.recent_keystrokes.len() > 4 {
            self.recent_keystrokes.remove(0);
        }
    }

    /// What the field holds.
    pub fn text(&self, cx: &App) -> String {
        self.text_input.read(cx).content.to_string()
    }

    /// The field's focus handle.
    pub fn text_input_focus(&self, cx: &App) -> FocusHandle {
        self.text_input.read(cx).focus_handle.clone()
    }
}
