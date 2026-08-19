use crate::gui::style::{
    AirshipperTheme, BRIGHT_ORANGE, DARK_WHITE, LIGHT_GREY, LILAC, TOMATO_RED,
};
use iced::{
    Color,
    widget::text::{Catalog, Style, StyleFn},
};

impl Catalog for AirshipperTheme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(normal)
    }

    fn style(&self, item: &Self::Class<'_>) -> Style {
        item(self)
    }
}

fn text_appearance(color: Color) -> Style {
    Style { color: Some(color) }
}

pub fn normal(_theme: &AirshipperTheme) -> Style {
    text_appearance(Color::WHITE)
}

pub fn dark(_theme: &AirshipperTheme) -> Style {
    text_appearance(DARK_WHITE)
}

pub fn light_grey(_theme: &AirshipperTheme) -> Style {
    text_appearance(LIGHT_GREY)
}

pub fn bright_orange(_theme: &AirshipperTheme) -> Style {
    text_appearance(BRIGHT_ORANGE)
}

pub fn tomato_red(_theme: &AirshipperTheme) -> Style {
    text_appearance(TOMATO_RED)
}

pub fn lilac(_theme: &AirshipperTheme) -> Style {
    text_appearance(LILAC)
}
