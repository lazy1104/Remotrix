use iced::widget::Space;
use iced::Length;

use crate::ui::dims::SPACE_LG;

pub fn view() -> Space {
    Space::new().height(Length::Fixed(SPACE_LG))
}
