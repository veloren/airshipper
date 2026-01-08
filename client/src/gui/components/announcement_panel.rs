use crate::{
    Result,
    assets::{POPPINS_MEDIUM_FONT, UP_RIGHT_ARROW_ICON},
    consts::{AIRSHIPPER_RELEASE_URL, SUPPORTED_SERVER_API_VERSION},
    gui::{
        style::{button::ButtonStyle, container::ContainerStyle, text::TextStyle},
        views::default::{DefaultViewMessage, Interaction},
        widget::*,
    },
    net,
};
use iced::{
    Alignment, Command, Length,
    alignment::Vertical,
    widget::{button, column, container, image, image::Handle, row, text},
};
use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Clone, Debug)]
pub enum AnnouncementPanelMessage {
    FetchAnnouncement(Result<AnnouncementPanelComponent>),
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct AnnouncementPanelComponent {
    pub announcement_message: Option<String>,
    pub announcement_last_change: chrono::DateTime<chrono::Utc>,
    pub api_version: Option<u32>,
}

impl AnnouncementPanelComponent {
    pub async fn fetch(
        api_version_url: String,
        announcement_url: String,
    ) -> Result<Self> {
        #[derive(Deserialize)]
        pub struct Version {
            version: u32,
        }

        #[derive(Deserialize)]
        pub struct Announcement {
            message: Option<String>,
            last_change: chrono::DateTime<chrono::Utc>,
        }

        debug!("Announcement fetching...");

        let version = net::query(api_version_url).await?.json::<Version>().await?;
        let announcement = net::query(announcement_url)
            .await?
            .json::<Announcement>()
            .await?;

        Ok(AnnouncementPanelComponent {
            announcement_message: announcement.message,
            announcement_last_change: announcement.last_change,
            api_version: Some(version.version),
        })
    }

    pub fn update(
        &mut self,
        msg: AnnouncementPanelMessage,
    ) -> Option<Command<DefaultViewMessage>> {
        match msg {
            AnnouncementPanelMessage::FetchAnnouncement(result) => match result {
                Ok(announcement) => {
                    *self = announcement;
                    None
                },
                Err(e) => {
                    tracing::trace!("Failed to fetch announcement: {}", e);
                    None
                },
            },
        }
    }

    pub fn view(&self) -> Element<'_, DefaultViewMessage> {
        let update = match self.api_version {
            Some(version) => SUPPORTED_SERVER_API_VERSION != version,
            None => false,
        };
        let rowtext = match (update, &self.announcement_message) {
            (false, None) => {
                return row![].into();
            },
            (true, None) => {
                "Airshipper is outdated, please update to the latest release!".to_string()
            },
            (false, Some(msg)) => {
                let date: chrono::DateTime<chrono::Local> =
                    self.announcement_last_change.into();
                format!("News from {}: {}", date.format("%Y-%m-%d %H:%M"), msg)
            },
            (true, Some(msg)) => {
                format!("Airshipper is outdated! News: {}", msg)
            },
        };

        let mut content_row = row![
            container(
                Text::new(rowtext)
                    .size(14)
                    .style(TextStyle::Dark)
                    .font(POPPINS_MEDIUM_FONT),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Vertical::Center)
            .padding([3, 0, 0, 16]),
        ];
        if update {
            content_row = content_row.push(
                container(
                    button(
                        row![
                            text("Download Airshipper").size(10),
                            image(Handle::from_memory(UP_RIGHT_ARROW_ICON.to_vec(),))
                        ]
                        .spacing(5)
                        .align_items(Alignment::Center),
                    )
                    .on_press(DefaultViewMessage::Interaction(Interaction::OpenURL(
                        AIRSHIPPER_RELEASE_URL.to_string(),
                    )))
                    .padding([4, 10, 0, 12])
                    .height(Length::Fixed(20.0))
                    .style(ButtonStyle::AirshipperDownload),
                )
                .padding([0, 20, 0, 0])
                .height(Length::Fill)
                .align_y(Vertical::Center)
                .width(Length::Shrink),
            );
        }

        let top_row = row![column![
            container(content_row.height(Length::Fill)).align_y(Vertical::Center),
        ]]
        .height(Length::Fixed(50.0));

        let col = column![].push(
            container(top_row)
                .width(Length::Fill)
                .style(ContainerStyle::Announcement),
        );

        let announcement_container = container(col);
        announcement_container.into()
    }
}
