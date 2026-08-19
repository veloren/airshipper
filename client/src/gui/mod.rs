pub mod components;
mod custom_widgets;
mod rss_feed;
mod style;
mod subscriptions;
mod views;
mod widget;

use std::borrow::Cow;

#[cfg(feature = "bundled_font")]
use crate::assets::UNIVERSAL_FONT_BYTES;
use crate::{
    Result,
    assets::{
        POPPINS_BOLD_FONT_BYTES, POPPINS_FONT_BYTES, POPPINS_LIGHT_FONT_BYTES,
        POPPINS_MEDIUM_FONT_BYTES,
    },
    cli::CmdLine,
    gui::{
        style::{AirshipperTheme, AirshipperThemeStyle},
        widget::*,
    },
    profiles::Profile,
};
use iced::{Settings, Size, Subscription, Task, widget::image as iced_image, window};
use icon::Icon;
#[cfg(windows)]
use views::update::{UpdateView, UpdateViewMessage};
use views::{
    Action, View,
    default::{DefaultView, DefaultViewMessage},
};

/// Starts the GUI and won't return unless an error occurs
pub fn run(cmd: CmdLine) -> Result<()> {
    let (iced_settings, window_settings) = settings(cmd);
    Ok(
        iced::application(Airshipper::boot, Airshipper::update, Airshipper::view)
            .subscription(Airshipper::subscription)
            .title(Airshipper::title)
            .theme(Airshipper::theme)
            .settings(iced_settings)
            .window(window_settings)
            .run()?,
    )
}

#[derive(Debug, Clone)]
pub struct Airshipper {
    view: View,

    pub default_view: DefaultView,
    #[cfg(windows)]
    update_view: UpdateView,
    pub active_profile: Profile,

    // Airshipper update
    #[cfg(windows)]
    update: Option<self_update::update::Release>,
}

#[allow(clippy::enum_variant_names, clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum Message {
    IconAllocated(std::result::Result<(Icon, iced_image::Allocation), iced_image::Error>),
    Loaded,
    #[allow(dead_code)]
    Saved(Result<()>),

    // Views
    DefaultViewMessage(DefaultViewMessage),
    #[cfg(windows)]
    UpdateViewMessage(UpdateViewMessage),
}

impl Airshipper {
    fn title(&self) -> String {
        format!("Airshipper v{}", env!("CARGO_PKG_VERSION"))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::IconAllocated(result) => {
                if let Ok(allocation) = result.map_err(|e| {
                    tracing::error!("Failed to allocate memory for an icon: {e}")
                }) {
                    let _ = icon::lock(allocation).map_err(|(icon, _)| {
                        tracing::debug!("Icon already locked: {icon:?}")
                    });
                }
            },
            Message::Loaded => {
                return self
                    .default_view
                    .update(DefaultViewMessage::Query, &self.active_profile)
                    .map(Message::DefaultViewMessage);
            },
            Message::Saved(_) => {},

            // Views
            Message::DefaultViewMessage(msg) => {
                if let DefaultViewMessage::Action(action) = &msg {
                    match action {
                        Action::UpdateProfile(profile) => {
                            self.active_profile = profile.clone();
                            self.active_profile.reload_wgpu_backends();
                            self.active_profile.reload_wgpu_devices();

                            return Task::perform(
                                Profile::save(self.active_profile.clone()),
                                Message::Saved,
                            );
                        },
                        #[cfg(windows)] // for now
                        Action::SwitchView(view) => self.view = *view,
                        #[cfg(windows)]
                        Action::LauncherUpdate(release) => {
                            self.update = Some(release.clone());
                            self.view = View::Update
                        },
                    }
                }

                return self
                    .default_view
                    .update(msg, &self.active_profile)
                    .map(Message::DefaultViewMessage);
            },
            #[cfg(windows)]
            Message::UpdateViewMessage(msg) => {
                if let UpdateViewMessage::Action(action) = &msg {
                    match action {
                        Action::UpdateProfile(profile) => {
                            self.active_profile = profile.clone();
                            return Task::perform(
                                Profile::save(self.active_profile.clone()),
                                Message::Saved,
                            );
                        },
                        Action::SwitchView(view) => self.view = *view,
                        Action::LauncherUpdate(_) => {},
                    }
                }

                return self
                    .update_view
                    .update(msg, &self.update)
                    .map(Message::UpdateViewMessage);
            },
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let Self {
            view, default_view, ..
        } = self;

        match view {
            View::Default => default_view
                .view(&self.active_profile)
                .map(Message::DefaultViewMessage),
            #[cfg(windows)]
            View::Update => self.update_view.view().map(Message::UpdateViewMessage),
        }
    }

    fn theme(&self) -> AirshipperTheme {
        AirshipperTheme {
            style: AirshipperThemeStyle::Default,
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        match self.view {
            View::Default => self
                .default_view
                .subscription()
                .map(Message::DefaultViewMessage),
            #[cfg(windows)]
            View::Update => iced::Subscription::none(),
        }
    }

    fn boot() -> (Self, Task<Message>) {
        #[cfg(windows)]
        crate::windows::hide_non_inherited_console();

        let active_profile = Profile::load();

        (
            Self {
                view: View::Default,
                default_view: DefaultView::default(),
                #[cfg(windows)]
                update_view: UpdateView::default(),
                active_profile,
                #[cfg(windows)]
                update: None,
            },
            Task::batch([
                Task::done(Message::Loaded),
                icon::batch().map(Message::IconAllocated),
            ]),
        )
    }

    const APP_ID: &'static str = "net.veloren.airshipper";
}

fn settings(_cmd: CmdLine) -> (Settings, window::Settings) {
    let icon = image::load_from_memory(crate::assets::VELOREN_ICON).unwrap();

    #[cfg_attr(not(target_os = "linux"), expect(unused_mut))]
    let mut window_settings = window::Settings {
        size: Size::new(1050.0, 720.0),
        resizable: true,
        decorations: true,
        icon: Some(
            window::icon::from_rgba(
                icon.to_rgba8().into_raw(),
                icon.width(),
                icon.height(),
            )
            .unwrap(),
        ),
        min_size: Some(Size::new(400.0, 250.0)),
        ..Default::default()
    };

    #[cfg(target_os = "linux")]
    {
        window_settings.platform_specific.application_id = Airshipper::APP_ID.to_string();
    }

    let iced_settings = Settings {
        default_font: crate::assets::POPPINS_FONT,
        default_text_size: 20.0.into(),
        id: Some(Airshipper::APP_ID.to_string()),
        fonts: vec![
            #[cfg(feature = "bundled_font")]
            Cow::Borrowed(UNIVERSAL_FONT_BYTES),
            Cow::Borrowed(POPPINS_FONT_BYTES),
            Cow::Borrowed(POPPINS_BOLD_FONT_BYTES),
            Cow::Borrowed(POPPINS_MEDIUM_FONT_BYTES),
            Cow::Borrowed(POPPINS_LIGHT_FONT_BYTES),
        ],
        ..Default::default()
    };

    (iced_settings, window_settings)
}
