use crate::gui::style::AirshipperTheme;
use iced::{
    Color,
    widget::{
        rule,
        rule::{Appearance, FillMode},
    },
};

#[derive(Debug, Clone, Copy, Default)]
pub enum RuleStyle {
    #[default]
    Default,
}

impl rule::StyleSheet for AirshipperTheme {
    type Style = RuleStyle;

    fn appearance(&self, style: &Self::Style) -> Appearance {
        match style {
            RuleStyle::Default => default_rule_style(),
        }
    }
}

fn default_rule_style() -> Appearance {
    Appearance {
        width: 1,
        color: Color::WHITE,
        radius: 0.0.into(),
        fill_mode: FillMode::Full,
    }
}
