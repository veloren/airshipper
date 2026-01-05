use crate::{
    Result,
    assets::{
        GLOBE_ICON, KEY_ICON, PING_ERROR_ICON, PING_NONE_ICON, PING1_ICON, PING2_ICON,
        PING3_ICON, PING4_ICON, POPPINS_BOLD_FONT, POPPINS_MEDIUM_FONT, STAR_ICON,
        UNIVERSAL_FONT, UP_RIGHT_ARROW_ICON,
    },
    consts,
    consts::{GITLAB_SERVER_BROWSER_URL, OFFICIAL_SERVER_LIST},
    gui::{
        components::GamePanelMessage,
        style::{
            button::{BrowserButtonStyle, ButtonStyle, ServerListEntryButtonState},
            container::ContainerStyle,
            text::TextStyle,
        },
        views::default::{DefaultViewMessage, Interaction},
        widget::*,
    },
    net,
    server_list::fetch_server_list,
};
use consts::OFFICIAL_AUTH_SERVER;
use iced::{
    Alignment, Command, Length,
    alignment::{Horizontal, Vertical},
    widget::{
        Image, button, column, container, horizontal_rule, image, image::Handle, row,
        scrollable, text, tooltip, tooltip::Position,
    },
};
use std::{borrow::Cow, cmp::min, time::Duration};
use tracing::debug;
use url::Url;
use veloren_query_server::{client::QueryClient, proto::ServerInfo as QueryServerInfo};
use veloren_serverbrowser_api::{FieldContent, GameServer};

pub const SERVER_BROWSER_PING_REFRESH: Duration = Duration::from_secs(20);

#[derive(Clone, Debug)]
pub struct ServerBrowserEntry {
    server: GameServer,
    ping: Option<Duration>,
    server_info: Option<QueryServerInfo>,
    query_client: SkipDebugClone<Option<QueryClient>>,
}

/// Newtype that skips debug and when the inner type is an option clones will always
/// result in `None`. Needed because `QueryClient` is neither of both.
pub struct SkipDebugClone<T>(pub T);
impl<T> core::fmt::Debug for SkipDebugClone<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkipDebug").finish()
    }
}
impl<T> Clone for SkipDebugClone<Option<T>> {
    fn clone(&self) -> Self {
        Self(None)
    }
}

impl From<GameServer> for ServerBrowserEntry {
    fn from(server: GameServer) -> Self {
        Self {
            server,
            ping: None,
            server_info: None,
            query_client: SkipDebugClone(None),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ServerBrowserPanelMessage {
    SelectServerEntry(Option<usize>),
    UpdateServerList(Result<Option<ServerBrowserPanelComponent>>),
    RefreshPing,
    UpdateServerPing {
        server_address: String,
        server_info: Option<QueryServerInfo>,
        ping: Option<Duration>,
        query_client: SkipDebugClone<Option<QueryClient>>,
    },
    SortServers(ServerSortOrder),
}

#[derive(Debug, Default, Clone)]
pub struct ServerBrowserPanelComponent {
    servers: Vec<ServerBrowserEntry>,
    selected_index: Option<usize>,
    server_list_fetch_error: bool,
    last_sort_ordering: Option<ServerSortOrder>,
}

impl ServerBrowserPanelComponent {
    pub(crate) async fn fetch() -> Result<Option<Self>> {
        let servers: Vec<ServerBrowserEntry>;
        let mut server_list_fetch_error = false;

        if let Ok(server_list) =
            fetch_server_list(format!("{}/v1/servers", OFFICIAL_SERVER_LIST).to_owned())
                .await
        {
            servers = server_list
                .servers
                .into_iter()
                .filter(|x| matches!(x.auth_server.as_str(), OFFICIAL_AUTH_SERVER))
                .map(ServerBrowserEntry::from)
                .collect();
        } else {
            servers = vec![];
            server_list_fetch_error = true;
        }

        Ok(Some(Self {
            servers,
            selected_index: None,
            last_sort_ordering: None,
            server_list_fetch_error,
        }))
    }

    pub fn view(&self) -> Element<'_, DefaultViewMessage> {
        let top_row = row![].height(Length::Fixed(50.0)).push(
            column![].push(container(
                row![]
                    .push(
                        container(
                            button(Image::new(Handle::from_memory(GLOBE_ICON.to_vec())))
                                .on_press(DefaultViewMessage::ServerBrowserPanel(
                                    ServerBrowserPanelMessage::RefreshPing,
                                )),
                        )
                        .center_x()
                        .center_y()
                        .height(Length::Fill)
                        .width(Length::Shrink)
                        .align_y(Vertical::Center)
                        .padding([0, 0, 0, 12]),
                    )
                    .push(
                        container(
                            text("Server Browser")
                                .style(TextStyle::Dark)
                                .size(16)
                                .font(POPPINS_MEDIUM_FONT),
                        )
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_y(Vertical::Center)
                        .padding([1, 0, 0, 8]),
                    )
                    .push(
                        container(
                            button(
                                row![]
                                    .push(text("Get your server listed here").size(10))
                                    .push(image(Handle::from_memory(
                                        UP_RIGHT_ARROW_ICON.to_vec(),
                                    )))
                                    .spacing(5)
                                    .align_items(Alignment::Center),
                            )
                            .on_press(DefaultViewMessage::Interaction(
                                Interaction::OpenURL(
                                    GITLAB_SERVER_BROWSER_URL.to_string(),
                                ),
                            ))
                            .padding([4, 10, 0, 10])
                            .height(Length::Fixed(20.0))
                            .style(ButtonStyle::Browser(BrowserButtonStyle::Gitlab)),
                        )
                        .height(Length::Fill)
                        .align_y(Vertical::Center)
                        .padding([1, 10, 0, 8]),
                    ),
            )),
        );

        let heading_button = |button_text: &str, sort_order: Option<ServerSortOrder>| {
            let mut button = button(
                text(button_text)
                    .font(POPPINS_BOLD_FONT)
                    .size(16)
                    .vertical_alignment(Vertical::Center),
            )
            .padding(0)
            .style(ButtonStyle::ColumnHeading);
            if let Some(order) = sort_order {
                button = button.on_press(DefaultViewMessage::ServerBrowserPanel(
                    ServerBrowserPanelMessage::SortServers(order),
                ))
            }
            button
        };

        const ICON_COLUMN_WIDTH: f32 = 35.0;
        let column_headings = container(
            row![]
                .width(Length::Fill)
                // Spacer heading for icons column
                .push(heading_button("", None).width(Length::Fixed(ICON_COLUMN_WIDTH)))
                .push(
                    heading_button("Server", Some(ServerSortOrder::ServerName))
                        .width(Length::FillPortion(3)),
                )
                .push(
                    heading_button("Location", Some(ServerSortOrder::Location))
                        .width(Length::FillPortion(2)),
                )
                .push(heading_button("Players", Some(ServerSortOrder::PlayerCount))
                    .width(Length::FillPortion(1))
                )
                .push(
                    heading_button("Ping (ms)", Some(ServerSortOrder::Ping))
                        .width(Length::FillPortion(1)),
                ),
        )
        .style(ContainerStyle::ColumnHeading)
        .padding([10, 8])
        .width(Length::Fill);

        let mut server_list = column![];

        let column_cell = |content: &str| {
            text(content)
                .width(Length::FillPortion(3))
                .font(UNIVERSAL_FONT)
                .height(Length::Fill)
                .size(14)
                .vertical_alignment(Vertical::Center)
        };

        for (i, server_entry) in self.servers.iter().enumerate() {
            let ping_icon = match server_entry.ping.map(|p| p.as_millis()) {
                Some(0..=50) => image(Handle::from_memory(PING1_ICON.to_vec())),
                Some(51..=150) => image(Handle::from_memory(PING2_ICON.to_vec())),
                Some(151..=300) => image(Handle::from_memory(PING3_ICON.to_vec())),
                Some(_) => image(Handle::from_memory(PING4_ICON.to_vec())),
                _ => {
                    if server_entry.server.query_port.is_none() {
                        image(Handle::from_memory(PING_NONE_ICON.to_vec()))
                    } else {
                        image(Handle::from_memory(PING_ERROR_ICON.to_vec()))
                    }
                },
            };

            let mut status_icons = row![]
                .spacing(5)
                .height(Length::Fill)
                .align_items(Alignment::Center);

            if !matches!(
                server_entry.server.auth_server.as_str(),
                OFFICIAL_AUTH_SERVER
            ) {
                status_icons = status_icons.push(
                    tooltip(
                        image(Handle::from_memory(KEY_ICON.to_vec()))
                            .height(Length::Fixed(16.0))
                            .width(Length::Fixed(16.0)),
                        text(
                            "This server is using a custom auth server. Do not log into \
                             this server unless you trust the owner.",
                        )
                        .size(14),
                        Position::Right,
                    )
                    .style(ContainerStyle::Tooltip)
                    .gap(5),
                );
            }

            if server_entry.server.official {
                status_icons = status_icons.push(
                    tooltip(
                        image(Handle::from_memory(STAR_ICON.to_vec()))
                            .height(Length::Fixed(16.0))
                            .width(Length::Fixed(16.0)),
                        text(
                            "This is an official server operated by the Veloren project",
                        )
                        .size(14),
                        Position::Right,
                    )
                    .style(ContainerStyle::Tooltip)
                    .gap(5),
                );
            }

            let row = row![]
                .width(Length::Fill)
                .align_items(Alignment::Center)
                .push(
                    container(status_icons)
                        .padding([0, 8])
                        .width(Length::Fixed(ICON_COLUMN_WIDTH))
                        .align_x(Horizontal::Right),
                )
                .push(
                    column_cell(
                        // Iced currently doesn't support truncating text widgets to
                        // prevent multi-line overflow so for now we truncate the server
                        // name to a length which doesn't wrap when the Airshipper window
                        // is at its default size
                        &server_entry.server.name
                            [..min(server_entry.server.name.len(), 40)],
                    )
                    .height(Length::Fill)
                    .width(Length::FillPortion(3)),
                )
                .push(
                    column_cell(
                        &server_entry
                            .server
                            .location
                            .as_ref()
                            .map_or("Unknown".to_owned(), |country| {
                                country.short_name.clone()
                            }),
                    )
                    .width(Length::FillPortion(2)),
                )
                .push(
                    column_cell(&server_entry.server_info.map_or(
                        Cow::Borrowed("?"),
                        |info| {
                            Cow::Owned(format!(
                                "{}/{}",
                                info.players_count, info.player_cap
                            ))
                        },
                    ))
                    .width(Length::FillPortion(1)),
                )
                .push(
                    row![]
                        .spacing(5)
                        .push(
                            container(ping_icon)
                                .height(Length::Fill)
                                .align_y(Vertical::Center),
                        )
                        .push(column_cell(&server_entry.ping.map_or_else(
                            || {
                                (if server_entry.server.query_port.is_none() {
                                    "?"
                                } else {
                                    "Error"
                                })
                                .to_owned()
                            },
                            |x| format!("{}", x.as_millis()),
                        )))
                        .width(Length::FillPortion(1)),
                )
                .padding(0);

            let row_style = if self
                .selected_index
                .is_some_and(|selected_index| selected_index == i)
            {
                ButtonStyle::ServerListEntry(ServerListEntryButtonState::Selected)
            } else {
                ButtonStyle::ServerListEntry(ServerListEntryButtonState::NotSelected)
            };
            let select_row_button = button(container(row).padding([0, 8]))
                .on_press(DefaultViewMessage::ServerBrowserPanel(
                    if self.selected_index == Some(i) {
                        ServerBrowserPanelMessage::SelectServerEntry(None)
                    } else {
                        ServerBrowserPanelMessage::SelectServerEntry(Some(i))
                    },
                ))
                .style(row_style)
                .height(Length::Fixed(30.0))
                .padding(0);

            server_list = server_list.push(select_row_button);
        }

        let mut col = column![].push(
            container(top_row)
                .width(Length::Fill)
                .height(Length::Shrink)
                .style(ContainerStyle::ChangelogHeader),
        );

        if !self.server_list_fetch_error {
            col = col
                .push(column_headings.height(Length::Shrink))
                .push(scrollable(server_list).height(Length::Fill));

            // If there's a selected server (which there should always be, unless the
            // server list API returned no servers) show details for that
            // server.
            let selected_server = self.selected_index.and_then(|x| self.servers.get(x));

            let discord_origin = url::Origin::Tuple(
                "https".to_string(),
                url::Host::Domain(String::from("discord.gg")),
                443,
            );
            let reddit_origin = url::Origin::Tuple(
                "https".to_string(),
                url::Host::Domain(String::from("reddit.com")),
                443,
            );
            let youtube_origin = url::Origin::Tuple(
                "https".to_string(),
                url::Host::Domain(String::from("youtube.com")),
                443,
            );

            if let Some(server) = selected_server {
                col = col
                    .push(
                        container(horizontal_rule(8))
                            .width(Length::Fill)
                            .padding([5, 20]),
                    )
                    .push(
                        container(scrollable(container({
                            let mut fields = server
                                .server
                                .extra
                                .clone()
                                .into_iter()
                                .collect::<Vec<_>>();
                            fields.sort_by(|a, b| a.0.cmp(&b.0));
                            let mut extras = row![].spacing(10);
                            for (id, field) in fields {
                                // TODO: Recognise common IDs, give them a custom icon
                                match field.content {
                                    FieldContent::Text(c) => {
                                        let container = match id.as_str() {
                                            "email" => container(
                                                text(format!("Email: {}", c)).size(12),
                                            )
                                            .padding([2, 10, 2, 10])
                                            .style(ContainerStyle::ExtraBrowser),
                                            _ => container(
                                                text(format!("{}: {}", field.name, c))
                                                    .size(14),
                                            )
                                            .padding([2, 10, 2, 10])
                                            .style(ContainerStyle::ExtraBrowser),
                                        };
                                        extras = extras.push(container);
                                    },
                                    FieldContent::Url(c) => {
                                        let mut button = button(
                                            row![]
                                                .push(text(field.name).size(12))
                                                .push(image(Handle::from_memory(
                                                    UP_RIGHT_ARROW_ICON.to_vec(),
                                                )))
                                                .spacing(5)
                                                .align_items(Alignment::Center),
                                        )
                                        .on_press(DefaultViewMessage::Interaction(
                                            Interaction::OpenURL(c.clone()),
                                        ))
                                        .padding([2, 10, 2, 10])
                                        .height(Length::Fixed(20.0));
                                        let button_style = match id.as_str() {
                                            "discord"
                                                if Url::parse(&c)
                                                    .map(|u| u.origin() == discord_origin)
                                                    .unwrap_or(false) =>
                                            {
                                                ButtonStyle::Browser(
                                                    BrowserButtonStyle::Discord,
                                                )
                                            },
                                            "reddit"
                                                if Url::parse(&c)
                                                    .map(|u| u.origin() == reddit_origin)
                                                    .unwrap_or(false) =>
                                            {
                                                ButtonStyle::Browser(
                                                    BrowserButtonStyle::Reddit,
                                                )
                                            },
                                            "youtube"
                                                if Url::parse(&c)
                                                    .map(|u| u.origin() == youtube_origin)
                                                    .unwrap_or(false) =>
                                            {
                                                ButtonStyle::Browser(
                                                    BrowserButtonStyle::Youtube,
                                                )
                                            },
                                            "mastodon" => ButtonStyle::Browser(
                                                BrowserButtonStyle::Mastodon,
                                            ),
                                            "twitch" => ButtonStyle::Browser(
                                                BrowserButtonStyle::Twitch,
                                            ),
                                            _ => ButtonStyle::Browser(
                                                BrowserButtonStyle::Extra,
                                            ),
                                        };
                                        button = button.style(button_style);
                                        extras = extras.push(button);
                                    },
                                    _ => {},
                                };
                            }
                            let queried_info =
                                if let Some(query_info) = &server.server_info {
                                    let battlemode = match  query_info.battlemode {
                                        veloren_query_server::proto::ServerBattleMode::GlobalPvP => "Global PvP",
                                        veloren_query_server::proto::ServerBattleMode::GlobalPvE => "Global PvE",
                                        veloren_query_server::proto::ServerBattleMode::PerPlayer => "Player selected",
                                    };

                                    column![
                                        text(format!("Battlemode: {battlemode}")).size(14),
                                        text(format!("Version: {:x}", query_info.git_hash)).size(14),
                                    ].spacing(5)
                                } else {
                                    column![text("Does not support the query server protocol :(").size(14)]
                                };

                            column![]
                                .spacing(5)
                                .width(Length::Fill)
                                .push(
                                    row![]
                                        .spacing(10)
                                        .push(
                                            text(&server.server.name)
                                                .font(UNIVERSAL_FONT)
                                                .size(14),
                                        )
                                        .push(
                                            text(display_gameserver_address(
                                                &server.server,
                                            ))
                                            .size(14)
                                            .font(UNIVERSAL_FONT)
                                            .style(TextStyle::BrightOrange),
                                        ),
                                )
                                .push(text("Description: ").font(UNIVERSAL_FONT).size(14))
                                .push(
                                    text(&server.server.description)
                                        .font(UNIVERSAL_FONT)
                                        .size(14),
                                )
                                .push(queried_info)
                                .push(extras)
                        }).width(Length::Fill)))
                        .height(Length::Fixed(160.0))
                        .padding([0, 0, 0, 40]),
                    );
            }
        } else {
            col = col.push(
                container(
                    text("Error fetching server list")
                        .size(14)
                        .style(TextStyle::TomatoRed),
                )
                .padding(20)
                .align_x(Horizontal::Center),
            )
        }

        let server_browser_container = container(col)
            .height(Length::Fill)
            .width(Length::Fill)
            .style(ContainerStyle::Dark);
        server_browser_container.into()
    }

    pub fn update(
        &mut self,
        msg: ServerBrowserPanelMessage,
    ) -> Option<Command<DefaultViewMessage>> {
        match msg {
            ServerBrowserPanelMessage::UpdateServerList(result) => match result {
                Ok(Some(server_browser)) => {
                    *self = server_browser;
                    if !self.servers.is_empty() {
                        // Why is there no simple `Command::message` ??
                        Some(Command::perform(async {}, |()| {
                            DefaultViewMessage::ServerBrowserPanel(
                                ServerBrowserPanelMessage::RefreshPing,
                            )
                        }))
                    } else {
                        None
                    }
                },
                Ok(None) => None,
                Err(e) => {
                    tracing::trace!("Failed to update server list: {}", e);
                    None
                },
            },
            ServerBrowserPanelMessage::UpdateServerPing {
                server_address,
                server_info,
                ping,
                query_client,
            } => {
                debug!(?ping, ?server_address, "Received ping result for server");

                if let Some(server) = self
                    .servers
                    .iter_mut()
                    .find(|x| x.server.address == server_address)
                {
                    server.ping = ping;
                    server.server_info = server_info;
                    server.query_client = query_client;
                };

                self.sort_servers(self.last_sort_ordering.unwrap_or_default());

                None
            },
            ServerBrowserPanelMessage::RefreshPing => Some(Command::batch(
                self.servers.iter_mut().filter_map(|server| {
                    let query_client = server.query_client.0.take();
                    let query_port = server.server.query_port?;
                    let server_address = server.server.address.clone();
                    let server_address2 = server.server.address.clone();

                    Some(Command::perform(
                        async move {
                            let mut query_client = match query_client {
                                Some(client) => client,
                                None => {
                                    crate::net::ping::create_client(
                                        &server_address2,
                                        query_port,
                                    )
                                    .await?
                                },
                            };
                            debug!(?server_address2, "Querying server");

                            let res =
                                crate::net::ping::perform_ping(&mut query_client).await;
                            Some((res, query_client))
                        },
                        move |res| {
                            let (query_client, server_info, ping) =
                                if let Some((res, query_client)) = res {
                                    let (ping, server_info) = res
                                        .inspect_err(|error| {
                                            debug!(
                                                ?server_address,
                                                ?error,
                                                "Failed to query server"
                                            )
                                        })
                                        .map_or((None, None), |(info, ping)| {
                                            (Some(info), Some(ping))
                                        });
                                    (Some(query_client), server_info, ping)
                                } else {
                                    (None, None, None)
                                };

                            DefaultViewMessage::ServerBrowserPanel(
                                ServerBrowserPanelMessage::UpdateServerPing {
                                    server_address,
                                    server_info,
                                    ping,
                                    query_client: SkipDebugClone(query_client),
                                },
                            )
                        },
                    ))
                }),
            )),
            ServerBrowserPanelMessage::SelectServerEntry(index) => {
                self.selected_index = index;
                let selected_server = index.and_then(|index| {
                    self.servers
                        .get(index)
                        .map(|x| display_gameserver_address(&x.server))
                });

                Some(Command::perform(async {}, move |()| {
                    DefaultViewMessage::GamePanel(
                        GamePanelMessage::ServerBrowserServerChanged(selected_server),
                    )
                }))
            },
            ServerBrowserPanelMessage::SortServers(order) => {
                self.sort_servers(order);
                self.last_sort_ordering = Some(order);
                None
            },
        }
    }

    fn sort_servers(&mut self, order: ServerSortOrder) {
        match order {
            ServerSortOrder::Default => self.servers.sort_unstable_by_key(|x| {
                (
                    !x.server.official,
                    x.ping.or(Some(Duration::MAX)),
                    x.server.name.clone(),
                )
            }),
            ServerSortOrder::PlayerCount => {
                self.servers.sort_unstable_by(|entry_a, entry_b| {
                    let cnt = |e: &ServerBrowserEntry| {
                        e.server_info.map(|info| info.players_count)
                    };

                    cnt(entry_b).cmp(&cnt(entry_a))
                })
            },
            ServerSortOrder::Ping => self
                .servers
                .sort_unstable_by_key(|x| x.ping.or(Some(Duration::MAX))),
            ServerSortOrder::ServerName => {
                self.servers.sort_unstable_by_key(|x| x.server.name.clone())
            },
            ServerSortOrder::Location => self.servers.sort_unstable_by_key(|x| {
                x.server
                    .location
                    .as_ref()
                    .map_or("".to_owned(), |country| country.short_name.clone())
            }),
        }
    }
}

fn display_gameserver_address(gameserver: &GameServer) -> String {
    if gameserver.port == net::DEFAULT_GAME_PORT {
        gameserver.address.clone()
    } else {
        format!("{}:{}", gameserver.address, gameserver.port)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum ServerSortOrder {
    #[default]
    Default,
    PlayerCount,
    ServerName,
    Location,
    Ping,
}
