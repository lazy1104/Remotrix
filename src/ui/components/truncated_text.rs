use iced::advanced::layout;
use iced::advanced::renderer;
use iced::advanced::text::paragraph::{self, Plain};
use iced::advanced::text::Renderer as TextRenderer;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::widget::Widget;
use iced::advanced::Layout;
use iced::widget::text::{self, Format, LineHeight, Shaping, Wrapping};
use iced::{alignment, mouse, Color, Element, Font, Length, Pixels, Rectangle, Size};

pub fn truncated_text(content: impl Into<String>) -> TruncatedText {
    TruncatedText::new(content)
}

pub struct TruncatedText<F = Font> {
    content: String,
    max_lines: u16,
    size: Option<Pixels>,
    font: Option<F>,
    color: Option<Color>,
    line_height: LineHeight,
    width: Length,
    wrapping: Wrapping,
}

impl<F> TruncatedText<F> {
    pub fn new(content: impl Into<String>) -> Self {
        TruncatedText {
            content: content.into(),
            max_lines: 2,
            size: None,
            font: None,
            color: None,
            line_height: LineHeight::default(),
            width: Length::Fill,
            wrapping: Wrapping::Glyph,
        }
    }

    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = Some(size.into());
        self
    }

    pub fn font(mut self, font: impl Into<F>) -> Self {
        self.font = Some(font.into());
        self
    }

    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn line_height(mut self, line_height: impl Into<LineHeight>) -> Self {
        self.line_height = line_height.into();
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn max_lines(mut self, max_lines: u16) -> Self {
        self.max_lines = max_lines;
        self
    }

    pub fn wrapping(mut self, wrapping: Wrapping) -> Self {
        self.wrapping = wrapping;
        self
    }
}

struct TruncState<P: paragraph::Paragraph> {
    paragraph: Plain<P>,
    last_input: String,
    last_width: f32,
    last_size: Pixels,
    last_font: Option<P::Font>,
    last_max_lines: u16,
    last_line_height: LineHeight,
    last_wrapping: Wrapping,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for TruncatedText<Renderer::Font>
where
    Renderer: TextRenderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<TruncState<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(TruncState::<Renderer::Paragraph> {
            paragraph: Plain::default(),
            last_input: String::new(),
            last_width: -1.0,
            last_size: Pixels(0.0),
            last_font: None,
            last_max_lines: 0,
            last_line_height: LineHeight::default(),
            last_wrapping: Wrapping::default(),
        })
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: Length::Shrink,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let TruncState {
            paragraph,
            last_input,
            last_width,
            last_size,
            last_font,
            last_max_lines,
            last_line_height,
            last_wrapping,
        } = tree.state.downcast_mut::<TruncState<Renderer::Paragraph>>();

        let size = self.size.unwrap_or_else(|| renderer.default_size());
        let font = self.font.unwrap_or_else(|| renderer.default_font());
        let max_height = self.line_height.to_absolute(size).0 * f32::from(self.max_lines);

        let format = Format {
            width: Length::Fill,
            height: Length::Shrink,
            size: Some(size),
            font: Some(font),
            line_height: self.line_height,
            align_x: text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            shaping: Shaping::Auto,
            wrapping: self.wrapping,
        };

        let key_width = (limits.max().width * 0.5).round() * 2.0;

        if *last_input == self.content
            && (key_width - *last_width).abs() < 0.5
            && *last_size == size
            && *last_font == self.font
            && *last_max_lines == self.max_lines
            && *last_line_height == self.line_height
            && *last_wrapping == self.wrapping
        {
            return layout::sized(limits, Length::Fill, Length::Shrink, |_| {
                paragraph.min_bounds()
            });
        }

        let node = text::layout(paragraph, renderer, limits, &self.content, format);

        if node.bounds().height <= max_height {
            *last_input = self.content.clone();
            *last_width = key_width;
            *last_size = size;
            *last_font = self.font;
            *last_max_lines = self.max_lines;
            *last_line_height = self.line_height;
            *last_wrapping = self.wrapping;
            return node;
        }

        let mut lo = 0usize;
        let mut hi = self.content.chars().count();

        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            let candidate = with_ellipsis(&self.content, mid);
            let candidate_node = text::layout(paragraph, renderer, limits, &candidate, format);
            if candidate_node.bounds().height <= max_height {
                lo = mid;
            } else {
                hi = mid;
            }
        }

        let display = with_ellipsis(&self.content, lo);
        let node = text::layout(paragraph, renderer, limits, &display, format);

        *last_input = self.content.clone();
        *last_width = key_width;
        *last_size = size;
        *last_font = self.font;
        *last_max_lines = self.max_lines;
        *last_line_height = self.line_height;
        *last_wrapping = self.wrapping;

        node
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        defaults: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<TruncState<Renderer::Paragraph>>();

        text::draw(
            renderer,
            defaults,
            layout.bounds(),
            state.paragraph.raw(),
            text::Style { color: self.color },
            viewport,
        );
    }
}

impl<'a, Message, Theme, Renderer> From<TruncatedText<Renderer::Font>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: TextRenderer + 'a,
{
    fn from(text: TruncatedText<Renderer::Font>) -> Self {
        Element::new(text)
    }
}

fn with_ellipsis(content: &str, count: usize) -> String {
    let mut result: String = content.chars().take(count).collect();
    result.push('…');
    result
}
