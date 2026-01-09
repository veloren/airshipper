//! Display an update dialog (windows only) to ask whether to update airshipper

use super::{Action, View};
use crate::gui::{
    style::{
        button::{ButtonState, ButtonStyle, DownloadButtonStyle},
        container::ContainerStyle,
    },
    widget::*,
};
use iced::{
    Alignment, Command, Length,
    alignment::{Horizontal, Vertical},
    widget::{button, column, container, row, text},
};
use self_update::update::Release;

#[derive(Debug, Clone)]
pub struct UpdateView {
    message: String,
}

impl Default for UpdateView {
    fn default() -> Self {
        Self {
            message: "Update for Airshipper available. Do you want to update?"
                .to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum UpdateViewMessage {
    // Messages
    Action(Action),

    // Updates
    LauncherUpdateFailed(String),

    // User Interactions
    UpdatePressed,
    SkipPressed,
}

impl UpdateView {
    pub fn view(&self) -> Element<'_, UpdateViewMessage> {
        // Contains everything
        let content = column![]
            .align_items(Alignment::Center)
            .spacing(10)
            .push(text(&self.message).size(14))
            .push(
                row![]
                    .align_items(Alignment::Center)
                    .spacing(100)
                    .padding(10)
                    .push(
                        button(
                            text("Skip")
                                .size(14)
                                .horizontal_alignment(Horizontal::Center)
                                .vertical_alignment(Vertical::Center),
                        )
                        .on_press(UpdateViewMessage::SkipPressed)
                        .style(ButtonStyle::Download(DownloadButtonStyle::Skip))
                        .width(Length::Fixed(100.0))
                        .height(Length::Fixed(35.0))
                        .padding(7),
                    )
                    .push(
                        button(
                            text("Update")
                                .size(14)
                                .width(Length::Fixed(90.0))
                                .horizontal_alignment(Horizontal::Center)
                                .vertical_alignment(Vertical::Center),
                        )
                        .on_press(UpdateViewMessage::UpdatePressed)
                        .style(ButtonStyle::Download(DownloadButtonStyle::Update(
                            ButtonState::Enabled,
                        )))
                        .width(Length::Fixed(100.0))
                        .height(Length::Fixed(35.0))
                        .padding(7),
                    ),
            );

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(ContainerStyle::Dark)
            .center_x()
            .center_y()
            .into()
    }

    pub fn update(
        &mut self,
        msg: UpdateViewMessage,
        release: &Option<Release>,
    ) -> Command<UpdateViewMessage> {
        match msg {
            // Will be handled by main view
            UpdateViewMessage::Action(_) => {},

            UpdateViewMessage::UpdatePressed => {
                tracing::info!("Updating Airshipper...");
                self.message = "Updating Airshipper...".to_string();
                let release = release.as_ref().unwrap().clone();
                return Command::perform(
                    async {
                        tokio::task::block_in_place(move || {
                            if let Err(e) = crate::windows::update(&release) {
                                tracing::error!("Failed to update Airshipper: {}", e);
                                return e.to_string();
                            }
                            String::new()
                        })
                    },
                    UpdateViewMessage::LauncherUpdateFailed, /* Update won't return
                                                              * except if update
                                                              * failed */
                );
            },

            UpdateViewMessage::LauncherUpdateFailed(reason) => {
                self.message = format!("Error: {}", reason);
            },

            UpdateViewMessage::SkipPressed => {
                return Command::perform(
                    async { Action::SwitchView(View::Default) },
                    UpdateViewMessage::Action,
                );
            },
        }

        Command::none()
    }
}
