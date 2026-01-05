use crate::gui::style::{
    AirshipperTheme, CORNFLOWER_BLUE, DARK_WHITE, LIGHT_GREY, MEDIUM_GREY, NAVY_BLUE,
};
use iced::{
    Background, Border, Color,
    widget::{text_input, text_input::Appearance},
};

#[derive(Default)]
pub enum TextInputStyle {
    #[default]
    Default,
}

impl text_input::StyleSheet for AirshipperTheme {
    type Style = TextInputStyle;

    fn active(&self, _: &Self::Style) -> Appearance {
        Appearance {
            background: Background::Color(NAVY_BLUE),
            border: Border {
                color: DARK_WHITE,
                width: 0.0,
                radius: 3.0.into(),
            },
            icon_color: Default::default(),
        }
    }

    fn focused(&self, style: &Self::Style) -> Appearance {
        self.active(style)
    }

    fn placeholder_color(&self, _: &Self::Style) -> Color {
        MEDIUM_GREY
    }

    fn value_color(&self, _: &Self::Style) -> Color {
        LIGHT_GREY
    }

    fn selection_color(&self, _: &Self::Style) -> Color {
        CORNFLOWER_BLUE
    }

    fn disabled_color(&self, _style: &Self::Style) -> Color {
        MEDIUM_GREY
    }

    fn disabled(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            background: Background::Color(MEDIUM_GREY),
            border: Border {
                color: DARK_WHITE,
                width: 0.0,
                radius: 3.0.into(),
            },
            icon_color: Default::default(),
        }
    }
}
