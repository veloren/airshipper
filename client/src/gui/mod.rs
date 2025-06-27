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
    gui::{style::AirshipperTheme, widget::*},
    profiles::Profile,
};
use iced::{Application, Command, Settings, Size, Subscription};
#[cfg(windows)]
use views::update::{UpdateView, UpdateViewMessage};
use views::{
    Action, View,
    default::{DefaultView, DefaultViewMessage},
};

/// Starts the GUI and won't return unless an error occurs
pub fn run(cmd: CmdLine) -> Result<()> {
    Ok(Airshipper::run(settings(cmd))?)
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

impl Airshipper {
    pub fn new(active_profile: Profile) -> Self {
        Self {
            view: View::default(),
            default_view: DefaultView::default(),
            #[cfg(windows)]
            update_view: UpdateView::default(),
            active_profile,
            #[cfg(windows)]
            update: None,
        }
    }
}

#[allow(clippy::enum_variant_names, clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum Message {
    Loaded,
    #[allow(dead_code)]
    Saved(Result<()>),

    // Views
    DefaultViewMessage(DefaultViewMessage),
    #[cfg(windows)]
    UpdateViewMessage(UpdateViewMessage),
}

impl Application for Airshipper {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = AirshipperTheme;
    type Flags = CmdLine;

    fn new(_flags: CmdLine) -> (Self, Command<Message>) {
        #[cfg(windows)]
        crate::windows::hide_non_inherited_console();

        (
            Airshipper::new(Profile::load()),
            Command::perform(async {}, |_| Message::Loaded),
        )
    }

    fn title(&self) -> String {
        format!("Airshipper v{}", env!("CARGO_PKG_VERSION"))
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
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

                            return Command::perform(
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
                            return Command::perform(
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

        Command::none()
    }

    fn view(&self) -> Element<Self::Message> {
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

    fn theme(&self) -> Self::Theme {
        AirshipperTheme {}
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
}

fn settings(cmd: CmdLine) -> Settings<CmdLine> {
    use iced::window::{Settings as Window, icon};
    let icon = image::load_from_memory(crate::assets::VELOREN_ICON).unwrap();

    Settings {
        window: Window {
            size: Size::new(1050.0, 720.0),
            resizable: true,
            decorations: true,
            icon: Some(
                icon::from_rgba(icon.to_rgba8().into_raw(), icon.width(), icon.height())
                    .unwrap(),
            ),
            min_size: Some(Size::new(400.0, 250.0)),
            ..Default::default()
        },
        flags: cmd,
        default_font: crate::assets::POPPINS_FONT,
        default_text_size: 20.0.into(),
        antialiasing: true,
        id: Some("airshipper".to_string()),
        fonts: vec![
            #[cfg(feature = "bundled_font")]
            Cow::Borrowed(UNIVERSAL_FONT_BYTES),
            Cow::Borrowed(POPPINS_FONT_BYTES),
            Cow::Borrowed(POPPINS_BOLD_FONT_BYTES),
            Cow::Borrowed(POPPINS_MEDIUM_FONT_BYTES),
            Cow::Borrowed(POPPINS_LIGHT_FONT_BYTES),
        ],
    }
}
