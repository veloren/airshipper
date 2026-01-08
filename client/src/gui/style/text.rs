use crate::gui::style::{
    AirshipperTheme, BRIGHT_ORANGE, DARK_WHITE, LIGHT_GREY, LILAC, TOMATO_RED,
};
use iced::{
    Color,
    widget::{text, text::Appearance},
};

#[derive(Debug, Clone, Copy, Default)]
pub enum TextStyle {
    #[default]
    Normal,
    Dark,
    LightGrey,
    BrightOrange,
    TomatoRed,
    Lilac,
}

impl text::StyleSheet for AirshipperTheme {
    type Style = TextStyle;

    fn appearance(&self, style: Self::Style) -> Appearance {
        match style {
            TextStyle::Normal => text_appearance(Color::WHITE),
            TextStyle::Dark => text_appearance(DARK_WHITE),
            TextStyle::LightGrey => text_appearance(LIGHT_GREY),
            TextStyle::BrightOrange => text_appearance(BRIGHT_ORANGE),
            TextStyle::TomatoRed => text_appearance(TOMATO_RED),
            TextStyle::Lilac => text_appearance(LILAC),
        }
    }
}

fn text_appearance(color: Color) -> Appearance {
    Appearance { color: Some(color) }
}
