use iced::widget::svg::{Handle, Style, Svg};
use iced::{Element, Length};

pub fn view<'a, Message: 'a>(
    theme: &'a iced::Theme,
    width: f32,
    height: f32,
) -> Element<'a, Message> {
    let primary = theme.extended_palette().primary.base.color;
    Svg::new(Handle::from_memory(include_bytes!(
        "../../../assets/logo.svg"
    )))
    .width(Length::Fixed(width))
    .height(Length::Fixed(height))
    .style(move |_t, _s| Style {
        color: Some(primary),
    })
    .into()
}
