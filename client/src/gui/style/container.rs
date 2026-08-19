use crate::gui::style::{
    AirshipperTheme, BACKGROUND_BLUE, BLOG_POST_BACKGROUND_BLUE, BRIGHT_ORANGE,
    DARK_WHITE, LIGHT_GREY, LIME_GREEN, MEDIUM_GREY, NAVY_BLUE, VERY_DARK_GREY,
};
use iced::{
    Border, Color,
    widget::container::{Catalog, Style, StyleFn},
};

impl Catalog for AirshipperTheme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|_| Style::default())
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

pub fn dark(_theme: &AirshipperTheme) -> Style {
    Style {
        background: Some(VERY_DARK_GREY.into()),
        text_color: Some(Color::WHITE),
        ..Style::default()
    }
}

pub fn announcement(_theme: &AirshipperTheme) -> Style {
    Style {
        background: Some(BRIGHT_ORANGE.into()),
        text_color: Some(Color::WHITE),
        ..Style::default()
    }
}

pub fn loading_blogpost(_theme: &AirshipperTheme) -> Style {
    Style {
        background: None,
        border: Border {
            color: DARK_WHITE,
            width: 0.7,
            ..Default::default()
        },
        text_color: Some(DARK_WHITE),
        ..Style::default()
    }
}

pub fn blogpost(_theme: &AirshipperTheme) -> Style {
    Style {
        background: Some(BLOG_POST_BACKGROUND_BLUE.into()),
        text_color: Some(Color::WHITE),
        ..Style::default()
    }
}

pub fn sidepanel(_theme: &AirshipperTheme) -> Style {
    Style {
        background: Some(BACKGROUND_BLUE.into()),
        ..Style::default()
    }
}

pub fn column_heading(_theme: &AirshipperTheme) -> Style {
    Style {
        text_color: Some(Color::WHITE),
        ..Style::default()
    }
}

pub fn changelog_header(_theme: &AirshipperTheme) -> Style {
    Style {
        background: Some(Color::BLACK.into()),
        text_color: Some(Color::WHITE),
        ..Style::default()
    }
}

pub fn extra_browser(_theme: &AirshipperTheme) -> Style {
    Style {
        background: Some(LIME_GREEN.into()),
        border: Border::default().rounded(25.0),
        ..Style::default()
    }
}

pub fn tooltip(_theme: &AirshipperTheme) -> Style {
    Style {
        text_color: Some(LIGHT_GREY),
        background: Some(NAVY_BLUE.into()),
        border: Border {
            color: MEDIUM_GREY,
            width: 1.0,
            ..Default::default()
        },
        ..Style::default()
    }
}
