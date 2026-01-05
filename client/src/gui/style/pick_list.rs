use crate::gui::style::{AirshipperTheme, NAVY_BLUE, VERY_DARK_GREY};
use iced::{
    Background, Border, Color,
    widget::{
        pick_list,
        pick_list::{Appearance, StyleSheet},
    },
};

#[derive(Copy, Clone, Debug, Default)]
pub enum PickListStyle {
    #[default]
    Default,
}

impl pick_list::StyleSheet for AirshipperTheme {
    type Style = PickListStyle;

    // TODO: menu from old picklist style?
    fn active(&self, _: &<Self as StyleSheet>::Style) -> Appearance {
        Appearance {
            text_color: Color::WHITE,
            background: Background::Color(NAVY_BLUE),
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

    fn hovered(&self, style: &<Self as StyleSheet>::Style) -> Appearance {
        self.active(style)
    }
}
