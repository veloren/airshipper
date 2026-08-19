#[cfg(windows)]
use crate::gui::style::TOMATO_RED;
use crate::gui::style::{
    AirshipperTheme, CORNFLOWER_BLUE, DARK_WHITE, DISCORD_BLURPLE, LIGHT_GREY,
    LIME_GREEN, MASTODON_PURPLE, NAVY_BLUE, REDDIT_ORANGE, SLATE, TRANSPARENT_WHITE,
    TWITCH_PURPLE, VERY_DARK_GREY, YOUTUBE_RED,
};
use iced::{
    Background, Border, Color, Shadow, Vector,
    widget::button::{Catalog, Status, Style, StyleFn},
};

impl Catalog for AirshipperTheme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(download_launch)
    }

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

pub fn download_launch(_theme: &AirshipperTheme, status: Status) -> Style {
    match status {
        Status::Active | Status::Pressed => active_download_button_style(LIME_GREEN),
        Status::Hovered => hovered_download_button_style(LIME_GREEN),
        Status::Disabled => disabled_download_button_style(),
    }
}

pub fn download_update(_theme: &AirshipperTheme, status: Status) -> Style {
    match status {
        Status::Active | Status::Pressed => active_download_button_style(CORNFLOWER_BLUE),
        Status::Hovered => hovered_download_button_style(CORNFLOWER_BLUE),
        Status::Disabled => disabled_download_button_style(),
    }
}

#[cfg(windows)]
pub fn download_skip(_theme: &AirshipperTheme, status: Status) -> Style {
    match status {
        Status::Active | Status::Pressed => active_download_button_style(TOMATO_RED),
        Status::Hovered => hovered_download_button_style(TOMATO_RED),
        Status::Disabled => disabled_download_button_style(),
    }
}

pub fn airshipper_download(_theme: &AirshipperTheme, _status: Status) -> Style {
    Style {
        background: Some(VERY_DARK_GREY.into()),
        border: Border::default().rounded(25.0),
        ..Style::default()
    }
}

pub fn server_list_entry_selected(_theme: &AirshipperTheme, status: Status) -> Style {
    match status {
        Status::Active | Status::Pressed | Status::Disabled => Style {
            background: Some(NAVY_BLUE.into()),
            text_color: Color::WHITE,
            ..Style::default()
        },
        Status::Hovered => Style {
            background: Some(NAVY_BLUE.into()),
            text_color: Color::WHITE,
            shadow: Shadow {
                offset: Vector::new(0.0, 0.0),
                ..Default::default()
            },
            ..Style::default()
        },
    }
}

pub fn server_list_entry_not_selected(_theme: &AirshipperTheme, status: Status) -> Style {
    match status {
        Status::Active | Status::Pressed | Status::Disabled => Style {
            background: Some(VERY_DARK_GREY.into()),
            text_color: Color::WHITE,
            ..Style::default()
        },
        Status::Hovered => Style {
            background: Some(color_multiply(VERY_DARK_GREY, 1.2).into()),
            text_color: Color::WHITE,
            shadow: Shadow {
                offset: Vector::new(0.0, 0.0),
                ..Default::default()
            },
            ..Style::default()
        },
    }
}

pub fn browser_gitlab(_theme: &AirshipperTheme, status: Status) -> Style {
    let color = match status {
        Status::Active | Status::Pressed | Status::Disabled => LIME_GREEN,
        Status::Hovered => color_multiply(LIME_GREEN, 1.1),
    };
    browser_color(status, color)
}

pub fn browser_discord(_theme: &AirshipperTheme, status: Status) -> Style {
    let color = match status {
        Status::Active | Status::Pressed | Status::Disabled => *DISCORD_BLURPLE,
        Status::Hovered => color_multiply(*DISCORD_BLURPLE, 1.1),
    };
    browser_color(status, color)
}

pub fn browser_mastodon(_theme: &AirshipperTheme, status: Status) -> Style {
    let color = match status {
        Status::Active | Status::Pressed | Status::Disabled => *MASTODON_PURPLE,
        Status::Hovered => color_multiply(*MASTODON_PURPLE, 1.1),
    };
    browser_color(status, color)
}

pub fn browser_reddit(_theme: &AirshipperTheme, status: Status) -> Style {
    let color = match status {
        Status::Active | Status::Pressed | Status::Disabled => *REDDIT_ORANGE,
        Status::Hovered => color_multiply(*REDDIT_ORANGE, 1.1),
    };
    browser_color(status, color)
}

pub fn browser_youtube(_theme: &AirshipperTheme, status: Status) -> Style {
    let color = match status {
        Status::Active | Status::Pressed | Status::Disabled => *YOUTUBE_RED,
        Status::Hovered => color_multiply(*YOUTUBE_RED, 1.1),
    };
    browser_color(status, color)
}

pub fn browser_twitch(_theme: &AirshipperTheme, status: Status) -> Style {
    let color = match status {
        Status::Active | Status::Pressed | Status::Disabled => *TWITCH_PURPLE,
        Status::Hovered => color_multiply(*TWITCH_PURPLE, 1.1),
    };
    browser_color(status, color)
}

pub fn browser_extra(_theme: &AirshipperTheme, status: Status) -> Style {
    let color = match status {
        Status::Active | Status::Pressed | Status::Disabled => LIME_GREEN,
        Status::Hovered => color_multiply(LIME_GREEN, 1.1),
    };
    browser_color(status, color)
}

fn browser_color(_status: Status, background: impl Into<Background>) -> Style {
    Style {
        background: Some(background.into()),
        border: Border::default().rounded(25.0),
        ..Style::default()
    }
}

fn active_download_button_style(background_color: Color) -> Style {
    Style {
        background: Some(background_color.into()),
        text_color: Color::WHITE,
        border: Border::default().rounded(4.0),
        ..Style::default()
    }
}

fn hovered_download_button_style(background_color: Color) -> Style {
    Style {
        background: Some(color_multiply(background_color, 1.1).into()),
        text_color: Color::WHITE,
        border: Border::default().rounded(4.0),
        ..Style::default()
    }
}

fn disabled_download_button_style() -> Style {
    Style {
        background: Some(SLATE.into()),
        shadow: Shadow {
            offset: Vector::new(1.0, 1.0),
            ..Default::default()
        },
        text_color: LIGHT_GREY,
        border: Border::default().rounded(4.0),
        ..Style::default()
    }
}

pub fn next_prev(_theme: &AirshipperTheme, _status: Status) -> Style {
    Style {
        background: None,
        text_color: DARK_WHITE,
        ..Style::default()
    }
}

pub fn transparent(_theme: &AirshipperTheme, _status: Status) -> Style {
    Style {
        background: None,
        ..Style::default()
    }
}

pub fn settings(_theme: &AirshipperTheme, status: Status) -> Style {
    match status {
        Status::Active | Status::Pressed | Status::Disabled => Style {
            background: Some(Color::TRANSPARENT.into()),
            border: Border::default().rounded(10.0),
            ..Style::default()
        },
        Status::Hovered => Style {
            background: Some(TRANSPARENT_WHITE.into()),
            border: Border::default().rounded(10.0),
            ..Style::default()
        },
    }
}

pub fn column_heading(_theme: &AirshipperTheme, _status: Status) -> Style {
    Style {
        text_color: Color::WHITE,
        ..Style::default()
    }
}

pub fn server_browser(_theme: &AirshipperTheme, status: Status) -> Style {
    let active = Style {
        background: Some(CORNFLOWER_BLUE.into()),
        text_color: Color::WHITE,
        border: Border::default().rounded(4.0),
        ..Style::default()
    };
    match status {
        Status::Active | Status::Pressed | Status::Disabled => active,
        Status::Hovered => Style {
            background: Some(color_multiply(CORNFLOWER_BLUE, 1.1).into()),
            ..active
        },
    }
}

fn color_multiply(color: Color, multiplier: f32) -> Color {
    Color::from_rgba(
        (color.r * multiplier).clamp(0.0, 1.0),
        (color.g * multiplier).clamp(0.0, 1.0),
        (color.b * multiplier).clamp(0.0, 1.0),
        color.a,
    )
}
