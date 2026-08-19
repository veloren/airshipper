use crate::gui::style::{ALMOST_BLACK, ALMOST_BLACK2, AirshipperTheme};
use iced::{
    Border, Color,
    widget::{
        container,
        scrollable::{AutoScroll, Catalog, Rail, Scroller, Status, Style, StyleFn},
    },
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
    let background = match status {
        Status::Active { .. } => Some(Color::TRANSPARENT.into()),
        Status::Hovered { .. } | Status::Dragged { .. } => Some(ALMOST_BLACK2.into()),
    };

    let rail = Rail {
        background,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        scroller: Scroller {
            border: Border {
                color: ALMOST_BLACK,
                width: 0.0,
                radius: 5.0.into(),
            },
            background: ALMOST_BLACK.into(),
        },
    };

    Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: AutoScroll {
            background: Color::TRANSPARENT.into(),
            border: Border {
                color: ALMOST_BLACK,
                width: 0.0,
                radius: 5.0.into(),
            },
            shadow: Default::default(),
            icon: ALMOST_BLACK,
        },
    }
}
