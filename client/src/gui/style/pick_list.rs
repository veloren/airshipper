use crate::gui::style::{AirshipperTheme, NAVY_BLUE, VERY_DARK_GREY};
use iced::{
    Border, Color,
    widget::pick_list::{Catalog, Status, Style, StyleFn},
};

impl Catalog for AirshipperTheme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> <Self as Catalog>::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &<Self as Catalog>::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

pub fn default(_theme: &AirshipperTheme, _status: Status) -> Style {
    Style {
        text_color: Color::WHITE,
        background: NAVY_BLUE.into(),
        // icon_size: 0.5, TODO: This was removed in a recent version of iced - the
        // dropdown handle should be smaller but this no longer appears possible.
        // Custom widget required?
        border: Border {
            width: 0.0,
            radius: 3.0.into(),
            color: Color::WHITE,
        },
        handle_color: Color::WHITE,
        placeholder_color: VERY_DARK_GREY,
    }
}
