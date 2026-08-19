use crate::gui::style::{
    AirshipperTheme, CORNFLOWER_BLUE, DARK_WHITE, LIGHT_GREY, MEDIUM_GREY, NAVY_BLUE,
};
use iced::{
    Border,
    widget::text_input::{Catalog, Status, Style, StyleFn},
};

impl Catalog for AirshipperTheme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

pub fn default(_theme: &AirshipperTheme, status: Status) -> Style {
    let style = Style {
        background: NAVY_BLUE.into(),
        border: Border {
            color: DARK_WHITE,
            width: 0.0,
            radius: 3.0.into(),
        },
        icon: Default::default(),
        placeholder: MEDIUM_GREY,
        value: LIGHT_GREY,
        selection: CORNFLOWER_BLUE,
    };
    match status {
        Status::Active | Status::Hovered | Status::Focused { .. } => style,
        Status::Disabled => Style {
            background: MEDIUM_GREY.into(),
            ..style
        },
    }
}
