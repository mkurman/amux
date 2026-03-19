//! Custom adapter that uses ftui markup parsing instead of Text::raw().
//!
//! The standard `StringModelAdapter` calls `Text::raw()` which treats
//! everything as plain text — ANSI escapes or markup tags appear as
//! literal characters. This adapter runs the view string through
//! `parse_markup()` so that `[fg=...]`, `[bg=...]`, `[bold]`, etc.
//! are interpreted as styled text.

use ftui_core::event::Event;
use ftui_render::cell::{Cell, CellContent};
use ftui_render::frame::Frame;
use ftui_runtime::program::{Cmd, Model};
use ftui_runtime::string_model::StringModel;
use ftui_text::markup::parse_markup;
use ftui_text::{Text, grapheme_width};
use unicode_segmentation::UnicodeSegmentation;

/// Adapter that bridges a [`StringModel`] to the full [`Model`] trait,
/// parsing the view string as ftui markup instead of raw text.
pub struct MarkupModelAdapter<S: StringModel> {
    inner: S,
}

impl<S: StringModel> MarkupModelAdapter<S> {
    /// Create a new adapter wrapping the given string model.
    #[inline]
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S: StringModel> Model for MarkupModelAdapter<S>
where
    S::Message: From<Event> + Send + 'static,
{
    type Message = S::Message;

    fn init(&mut self) -> Cmd<Self::Message> {
        self.inner.init()
    }

    fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
        self.inner.update(msg)
    }

    fn view(&self, frame: &mut Frame) {
        let s = self.inner.view_string();
        let text = match parse_markup(&s) {
            Ok(t) => t,
            Err(_) => Text::raw(&s),
        };
        render_text_to_frame(&text, frame);
    }
}

/// Render a `Text` into a `Frame`, line by line with span styles.
///
/// This is a copy of the private `render_text_to_frame` from
/// `ftui_runtime::string_model`, adapted for our use.
fn render_text_to_frame(text: &Text, frame: &mut Frame) {
    let width = frame.width();
    let height = frame.height();

    for (y, line) in text.lines().iter().enumerate() {
        if y as u16 >= height {
            break;
        }

        let mut x: u16 = 0;
        for span in line.spans() {
            if x >= width {
                break;
            }

            let style = span.style.unwrap_or_default();

            for grapheme in span.content.graphemes(true) {
                if x >= width {
                    break;
                }

                let w = grapheme_width(grapheme);
                if w == 0 {
                    continue;
                }

                // Skip if the wide character would exceed the buffer width
                if x + w as u16 > width {
                    break;
                }

                let content = if w > 1 || grapheme.chars().count() > 1 {
                    let id = frame.intern_with_width(grapheme, w as u8);
                    CellContent::from_grapheme(id)
                } else if let Some(c) = grapheme.chars().next() {
                    CellContent::from_char(c)
                } else {
                    continue;
                };

                let mut cell = Cell::new(content);
                apply_style(&mut cell, style);
                frame.buffer.set(x, y as u16, cell);

                x = x.saturating_add(w as u16);
            }
        }
    }
}

/// Apply a style to a cell.
fn apply_style(cell: &mut Cell, style: ftui_style::Style) {
    if let Some(fg) = style.fg {
        cell.fg = fg;
    }
    if let Some(bg) = style.bg {
        cell.bg = bg;
    }
    if let Some(attrs) = style.attrs {
        let cell_flags: ftui_render::cell::StyleFlags = attrs.into();
        cell.attrs = cell.attrs.with_flags(cell_flags);
    }
}
