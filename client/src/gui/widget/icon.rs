use crate::assets::{
    BOOK_ICON, CHANGELOG_ICON, CHAT_ICON, DOWNLOAD_ICON, FOLDER_ICON, GLOBE_ICON,
    HEART_ICON, KEY_ICON, PING_ERROR_ICON, PING_NONE_ICON, PING1_ICON, PING2_ICON,
    PING3_ICON, PING4_ICON, SETTINGS_ICON, STAR_ICON, UP_RIGHT_ARROW_ICON, USER_ICON,
    VELOREN_LOGO,
};

use iced::{
    Task,
    widget::{Image, image},
};

use std::sync::OnceLock;

pub fn book() -> Option<Image> {
    BOOK.get().map(image::Allocation::handle).map(image)
}

pub fn chat() -> Option<Image> {
    CHAT.get().map(image::Allocation::handle).map(image)
}

pub fn changelog() -> Option<Image> {
    CHANGELOG.get().map(image::Allocation::handle).map(image)
}

pub fn download() -> Option<Image> {
    DOWNLOAD.get().map(image::Allocation::handle).map(image)
}

pub fn folder() -> Option<Image> {
    FOLDER.get().map(image::Allocation::handle).map(image)
}

pub fn globe() -> Option<Image> {
    GLOBE.get().map(image::Allocation::handle).map(image)
}

pub fn heart() -> Option<Image> {
    HEART.get().map(image::Allocation::handle).map(image)
}

pub fn key() -> Option<Image> {
    KEY.get().map(image::Allocation::handle).map(image)
}

pub fn ping_one() -> Option<Image> {
    PING_ONE.get().map(image::Allocation::handle).map(image)
}

pub fn ping_two() -> Option<Image> {
    PING_TWO.get().map(image::Allocation::handle).map(image)
}

pub fn ping_three() -> Option<Image> {
    PING_THREE.get().map(image::Allocation::handle).map(image)
}

pub fn ping_four() -> Option<Image> {
    PING_FOUR.get().map(image::Allocation::handle).map(image)
}

pub fn ping_none() -> Option<Image> {
    PING_NONE.get().map(image::Allocation::handle).map(image)
}

pub fn ping_error() -> Option<Image> {
    PING_ERROR.get().map(image::Allocation::handle).map(image)
}

pub fn settings() -> Option<Image> {
    SETTINGS.get().map(image::Allocation::handle).map(image)
}

pub fn star() -> Option<Image> {
    STAR.get().map(image::Allocation::handle).map(image)
}

pub fn up_right_arrow() -> Option<Image> {
    UP_RIGHT_ARROW
        .get()
        .map(image::Allocation::handle)
        .map(image)
}

pub fn user() -> Option<Image> {
    USER.get().map(image::Allocation::handle).map(image)
}

pub fn veloren_logo() -> Option<Image> {
    VELOREN.get().map(image::Allocation::handle).map(image)
}

/// Used to identify [`image::Allocation`]s from a [`batch`] and [`lock`] them for
/// convenience
#[derive(Debug, Clone, Copy)]
pub enum Icon {
    Book,
    Chat,
    Changelog,
    Download,
    Folder,
    Globe,
    Heart,
    Key,
    PingOne,
    PingTwo,
    PingThree,
    PingFour,
    PingNone,
    PingError,
    Settings,
    Star,
    UpRightArrow,
    User,
    VelorenLogo,
}

/// Batch image allocations at app startup for re-use across app views
pub fn batch() -> Task<Result<(Icon, image::Allocation), image::Error>> {
    const ICONS: &[(Icon, &[u8])] = &[
        (Icon::Book, BOOK_ICON),
        (Icon::Chat, CHAT_ICON),
        (Icon::Changelog, CHANGELOG_ICON),
        (Icon::Download, DOWNLOAD_ICON),
        (Icon::Folder, FOLDER_ICON),
        (Icon::Heart, HEART_ICON),
        (Icon::Globe, GLOBE_ICON),
        (Icon::Key, KEY_ICON),
        (Icon::PingOne, PING1_ICON),
        (Icon::PingTwo, PING2_ICON),
        (Icon::PingThree, PING3_ICON),
        (Icon::PingFour, PING4_ICON),
        (Icon::PingNone, PING_NONE_ICON),
        (Icon::PingError, PING_ERROR_ICON),
        (Icon::Settings, SETTINGS_ICON),
        (Icon::Star, STAR_ICON),
        (Icon::UpRightArrow, UP_RIGHT_ARROW_ICON),
        (Icon::User, USER_ICON),
        (Icon::VelorenLogo, VELOREN_LOGO),
    ];

    Task::batch(ICONS.iter().map(|(icon, data)| {
        image::allocate(image::Handle::from_bytes(*data))
            .map(|result| result.map(|allocation| (*icon, allocation)))
    }))
}

/// Keep the [`image::Allocation`] of the [`Icon`] alive for duration of the app's
/// lifetime
pub fn lock(
    (icon, allocation): (Icon, image::Allocation),
) -> Result<(), (Icon, image::Allocation)> {
    match icon {
        Icon::Book => BOOK.set(allocation),
        Icon::Chat => CHAT.set(allocation),
        Icon::Changelog => CHANGELOG.set(allocation),
        Icon::Download => DOWNLOAD.set(allocation),
        Icon::Folder => FOLDER.set(allocation),
        Icon::Globe => GLOBE.set(allocation),
        Icon::Heart => HEART.set(allocation),
        Icon::Key => KEY.set(allocation),
        Icon::PingOne => PING_ONE.set(allocation),
        Icon::PingTwo => PING_TWO.set(allocation),
        Icon::PingThree => PING_THREE.set(allocation),
        Icon::PingFour => PING_FOUR.set(allocation),
        Icon::PingNone => PING_NONE.set(allocation),
        Icon::PingError => PING_ERROR.set(allocation),
        Icon::Settings => SETTINGS.set(allocation),
        Icon::Star => STAR.set(allocation),
        Icon::UpRightArrow => UP_RIGHT_ARROW.set(allocation),
        Icon::User => USER.set(allocation),
        Icon::VelorenLogo => VELOREN.set(allocation),
    }
    .map_err(|allocation| (icon, allocation))
}

static BOOK: OnceLock<image::Allocation> = OnceLock::new();
static CHAT: OnceLock<image::Allocation> = OnceLock::new();
static CHANGELOG: OnceLock<image::Allocation> = OnceLock::new();
static DOWNLOAD: OnceLock<image::Allocation> = OnceLock::new();
static FOLDER: OnceLock<image::Allocation> = OnceLock::new();
static GLOBE: OnceLock<image::Allocation> = OnceLock::new();
static HEART: OnceLock<image::Allocation> = OnceLock::new();
static KEY: OnceLock<image::Allocation> = OnceLock::new();
static PING_ONE: OnceLock<image::Allocation> = OnceLock::new();
static PING_TWO: OnceLock<image::Allocation> = OnceLock::new();
static PING_THREE: OnceLock<image::Allocation> = OnceLock::new();
static PING_FOUR: OnceLock<image::Allocation> = OnceLock::new();
static PING_NONE: OnceLock<image::Allocation> = OnceLock::new();
static PING_ERROR: OnceLock<image::Allocation> = OnceLock::new();
static SETTINGS: OnceLock<image::Allocation> = OnceLock::new();
static STAR: OnceLock<image::Allocation> = OnceLock::new();
static UP_RIGHT_ARROW: OnceLock<image::Allocation> = OnceLock::new();
static USER: OnceLock<image::Allocation> = OnceLock::new();
static VELOREN: OnceLock<image::Allocation> = OnceLock::new();
