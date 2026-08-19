use crate::gui::style::{AirshipperTheme, LIGHT_NAVY_BLUE, NAVY_BLUE};
use iced::{
    Border, Color,
    overlay::menu::{Catalog, Style, StyleFn},
};

impl Catalog for AirshipperTheme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> <Self as Catalog>::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &<Self as Catalog>::Class<'_>) -> Style {
        class(self)
    }
}

pub fn default(_theme: &AirshipperTheme) -> Style {
    Style {
        text_color: Color::WHITE,
        background: NAVY_BLUE.into(),
        selected_background: LIGHT_NAVY_BLUE.into(),
        selected_text_color: Color::WHITE,
        border: Border {
            color: Color::WHITE,
            width: 0.0,
            radius: 0.0.into(),
        },
        shadow: Default::default(),
    }
}
