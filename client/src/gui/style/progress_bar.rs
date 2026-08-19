use crate::gui::style::{AirshipperTheme, LIME_GREEN, VERY_DARK_GREY};
use iced::{
    Border,
    widget::progress_bar::{Catalog, Style, StyleFn},
};

impl Catalog for AirshipperTheme {
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
        background: VERY_DARK_GREY.into(),
        bar: LIME_GREEN.into(),
        border: Border {
            radius: 3.0.into(),
            ..Default::default()
        },
    }
}
