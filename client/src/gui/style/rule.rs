use crate::gui::style::AirshipperTheme;
use iced::{
    Color,
    widget::{
        rule,
        rule::{FillMode, Style, StyleFn},
    },
};

impl rule::Catalog for AirshipperTheme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

pub fn default(_theme: &AirshipperTheme) -> Style {
    Style {
        color: Color::WHITE,
        radius: 0.0.into(),
        fill_mode: FillMode::Full,
        snap: true,
    }
}
