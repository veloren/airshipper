use crate::{
    channels::Channels,
    gui::{
        components::{
            AnnouncementPanelComponent, AnnouncementPanelMessage,
            ChangelogPanelComponent, ChangelogPanelMessage, CommunityShowcaseComponent,
            CommunityShowcasePanelMessage, GamePanelComponent, GamePanelMessage,
            LogoPanelComponent, NewsPanelComponent, NewsPanelMessage,
            SERVER_BROWSER_PING_REFRESH, ServerBrowserPanelComponent,
            ServerBrowserPanelMessage, SettingsPanelComponent, SettingsPanelMessage,
        },
        rss_feed::RssFeedComponentMessage::UpdateRssFeed,
        style::container::ContainerStyle,
        subscriptions,
        views::Action,
        widget::*,
    },
    profiles::Profile,
};

use iced::{
    Command, Length,
    widget::{column, container, row},
};

#[cfg(windows)]
use crate::gui::Result;

#[derive(Default, Debug, Clone)]
pub struct DefaultView {
    changelog_panel_component: ChangelogPanelComponent,
    announcement_panel_component: AnnouncementPanelComponent,
    logo_panel_component: LogoPanelComponent,
    community_showcase_component: CommunityShowcaseComponent,
    game_panel_component: GamePanelComponent,
    news_panel_component: NewsPanelComponent,
    settings_panel_component: SettingsPanelComponent,
    server_browser_panel_component: ServerBrowserPanelComponent,
    show_settings: bool,
    show_server_browser: bool,
}

#[derive(Clone, Debug)]
pub enum DefaultViewMessage {
    // Messages
    Action(Action),
    Query,

    #[cfg(windows)]
    LauncherUpdate(Result<Option<self_update::update::Release>>),

    // User Interactions
    Interaction(Interaction),

    // Panel-specific messages
    GamePanel(GamePanelMessage),
    ChangelogPanel(ChangelogPanelMessage),
    AnnouncementPanel(AnnouncementPanelMessage),
    CommunityShowcasePanel(CommunityShowcasePanelMessage),
    NewsPanel(NewsPanelMessage),
    SettingsPanel(SettingsPanelMessage),
    ServerBrowserPanel(ServerBrowserPanelMessage),
}

#[derive(Debug, Clone)]
pub enum Interaction {
    SettingsPressed,
    ToggleServerBrowser,
    OpenURL(String),
}

impl DefaultView {
    pub fn subscription(&self) -> iced::Subscription<DefaultViewMessage> {
        iced::Subscription::batch(
            IntoIterator::into_iter([
                Some(
                    self.game_panel_component
                        .subscription()
                        .map(DefaultViewMessage::GamePanel),
                ),
                self.show_server_browser.then_some(
                    subscriptions::repeat_message::stream(
                        SERVER_BROWSER_PING_REFRESH,
                        DefaultViewMessage::ServerBrowserPanel(
                            ServerBrowserPanelMessage::RefreshPing,
                        ),
                    ),
                ),
            ])
            .flatten(),
        )
    }

    pub fn view<'a>(
        &'a self,
        active_profile: &'a Profile,
    ) -> Element<'a, DefaultViewMessage> {
        let Self {
            changelog_panel_component,
            announcement_panel_component,
            news_panel_component,
            logo_panel_component,
            community_showcase_component,
            game_panel_component,
            settings_panel_component,
            server_browser_panel_component,
            ..
        } = self;

        let left_middle_contents = if self.show_settings {
            settings_panel_component.view(active_profile)
        } else {
            community_showcase_component.view()
        };

        let left = container(
            column![]
                .push(container(logo_panel_component.view()).height(Length::Fill))
                .push(container(left_middle_contents).height(Length::Shrink))
                .push(
                    container(game_panel_component.view(active_profile))
                        .height(Length::Shrink),
                ),
        )
        .height(Length::Fill)
        .width(Length::Fixed(360.0))
        .style(ContainerStyle::SidePanel);

        let mut main_row = row![].push(left);

        if !self.show_server_browser {
            let middle = container(
                column![]
                    .push(
                        container(announcement_panel_component.view())
                            .height(Length::Shrink),
                    )
                    .push(
                        container(changelog_panel_component.view()).height(Length::Fill),
                    ),
            )
            .height(Length::Fill)
            .width(Length::Fill);
            let right = container(news_panel_component.view())
                .height(Length::Fill)
                .width(Length::Fixed(248.0))
                .style(ContainerStyle::SidePanel);

            main_row = main_row.push(middle).push(right);
        } else {
            let server_browser = container(server_browser_panel_component.view())
                .height(Length::Fill)
                .width(Length::Fill);
            main_row = main_row.push(server_browser);
        }

        container(main_row)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn update(
        &mut self,
        msg: DefaultViewMessage,
        active_profile: &Profile,
    ) -> Command<DefaultViewMessage> {
        match msg {
            // Messages
            // Will be handled by main view
            DefaultViewMessage::Action(_) => {},
            DefaultViewMessage::Query => {
                let channel = active_profile.channel.clone();
                let api_version_url = active_profile.api_version_url();
                let announcement_url = active_profile.announcement_url();
                return Command::batch(vec![
                    Command::perform(NewsPanelComponent::load_news(), |update| {
                        DefaultViewMessage::NewsPanel(NewsPanelMessage::RssUpdate(
                            UpdateRssFeed(update),
                        ))
                    }),
                    Command::perform(
                        ChangelogPanelComponent::load_changelog(),
                        move |update| {
                            DefaultViewMessage::ChangelogPanel(
                                ChangelogPanelMessage::LoadChangelog(update, channel),
                            )
                        },
                    ),
                    Command::perform(ServerBrowserPanelComponent::fetch(), |update| {
                        DefaultViewMessage::ServerBrowserPanel(
                            ServerBrowserPanelMessage::UpdateServerList(update),
                        )
                    }),
                    Command::perform(
                        AnnouncementPanelComponent::load_announcement(),
                        move |update| {
                            DefaultViewMessage::AnnouncementPanel(
                                AnnouncementPanelMessage::LoadAnnouncement(
                                    update,
                                    api_version_url,
                                    announcement_url,
                                ),
                            )
                        },
                    ),
                    Command::perform(
                        CommunityShowcaseComponent::load_community_posts(),
                        |update| {
                            DefaultViewMessage::CommunityShowcasePanel(
                                CommunityShowcasePanelMessage::RssUpdate(UpdateRssFeed(
                                    update,
                                )),
                            )
                        },
                    ),
                    Command::perform(
                        Channels::fetch(active_profile.channel_url()),
                        |channels| {
                            DefaultViewMessage::SettingsPanel(
                                SettingsPanelMessage::ChannelsLoaded(channels),
                            )
                        },
                    ),
                    #[cfg(windows)]
                    Command::perform(
                        async { tokio::task::block_in_place(crate::windows::query) },
                        DefaultViewMessage::LauncherUpdate,
                    ),
                    Command::perform(async {}, |_| {
                        DefaultViewMessage::GamePanel(GamePanelMessage::StartUpdate)
                    }),
                ]);
            },

            DefaultViewMessage::GamePanel(msg) => {
                if let Some(command) =
                    self.game_panel_component.update(msg, active_profile)
                {
                    return command;
                }
            },
            DefaultViewMessage::ChangelogPanel(msg) => {
                if let Some(command) = self.changelog_panel_component.update(msg) {
                    return command;
                }
            },
            DefaultViewMessage::AnnouncementPanel(msg) => {
                if let Some(command) = self.announcement_panel_component.update(msg) {
                    return command;
                }
            },
            DefaultViewMessage::CommunityShowcasePanel(msg) => {
                if let Some(command) = self.community_showcase_component.update(msg) {
                    return command;
                }
            },
            DefaultViewMessage::NewsPanel(msg) => {
                if let Some(command) = self.news_panel_component.update(msg) {
                    return command;
                }
            },

            DefaultViewMessage::SettingsPanel(msg) => {
                if let Some(command) =
                    self.settings_panel_component.update(msg, active_profile)
                {
                    return command;
                }
            },
            DefaultViewMessage::ServerBrowserPanel(msg) => {
                if let Some(command) = self.server_browser_panel_component.update(msg) {
                    return command;
                }
            },

            #[cfg(windows)]
            DefaultViewMessage::LauncherUpdate(update) => {
                if let Ok(Some(release)) = update {
                    return Command::perform(
                        async { Action::LauncherUpdate(release) },
                        DefaultViewMessage::Action,
                    );
                }
            },

            // User Interaction
            DefaultViewMessage::Interaction(interaction) => match interaction {
                Interaction::SettingsPressed => {
                    self.show_settings = !self.show_settings;
                },
                Interaction::ToggleServerBrowser => {
                    self.show_server_browser = !self.show_server_browser;

                    // If toggling the server browser panel resulted in it being hidden,
                    // deselect the selected server to switch the
                    // Launch button back to saying "Launch" instead of "Connect to
                    // selected server"
                    if !self.show_server_browser {
                        return Command::perform(async {}, |_| {
                            DefaultViewMessage::ServerBrowserPanel(
                                ServerBrowserPanelMessage::SelectServerEntry(None),
                            )
                        });
                    }
                },
                Interaction::OpenURL(url) => {
                    if let Err(e) = opener::open(url) {
                        tracing::error!(
                            "Failed to open gitlab changelog website: {:?}",
                            e
                        );
                    }
                },
            },
        }

        Command::none()
    }
}
