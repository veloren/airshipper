use crate::{
    Result,
    assets::{
        CHANGELOG_ICON, POPPINS_BOLD_FONT, POPPINS_LIGHT_FONT, POPPINS_MEDIUM_FONT,
        UP_RIGHT_ARROW_ICON,
    },
    channels::Channel,
    consts,
    consts::GITLAB_MERGED_MR_URL,
    gui::{
        style::{
            button::{BrowserButtonStyle, ButtonStyle},
            container::ContainerStyle,
            text::TextStyle,
        },
        views::default::{DefaultViewMessage, Interaction},
        widget::*,
    },
    net,
};
use iced::{
    Alignment, Command, Length,
    alignment::{Horizontal, Vertical},
    widget::{
        Image, button, column, container, image, image::Handle, row, scrollable, text,
        text::LineHeight,
    },
};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ron::{
    de::from_str,
    ser::{PrettyConfig, to_string_pretty},
};
use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Clone, Debug)]
pub enum ChangelogPanelMessage {
    ScrollPositionChanged(f32),
    LoadChangelog(Result<ChangelogPanelComponent>, Channel),
    UpdateChangelog(Result<Option<ChangelogPanelComponent>>),
    SaveChangelog,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogPanelComponent {
    // TODO: Separate the Changelog data from the Panel data to avoid replacing the whole
    // panel when the changelog is updated
    pub versions: Vec<ChangelogVersion>,
    pub etag: String,
    #[serde(skip, default = "default_display_count")]
    pub display_count: usize,
}

pub fn default_display_count() -> usize {
    2
}

impl ChangelogPanelComponent {
    #[allow(clippy::while_let_on_iterator)]
    async fn fetch(channel: Channel) -> Result<Option<Self>> {
        let mut versions: Vec<ChangelogVersion> = Vec::new();

        let changelog =
            net::query(consts::CHANGELOG_URL.replace("{tag}", &channel.0)).await?;
        let etag = net::get_etag(&changelog);

        let changelog_text = changelog.text().await?;
        let options = Options::empty();
        let mut parser = Parser::new_ext(changelog_text.as_str(), options).peekable();

        while let Some(event) = parser.next() {
            // h2 version header
            // starts a new version
            if let Event::Start(Tag::Heading {
                level: HeadingLevel::H2,
                ..
            }) = event
            {
                let mut version: String = String::new();
                let mut date: Option<String> = None;

                // h2 version header text
                while let Some(event) = parser.next() {
                    match event {
                        Event::End(TagEnd::Heading(HeadingLevel::H2)) => break,
                        Event::Text(text) => {
                            if text.contains(" - ") {
                                date = Some(text[3..].trim().to_string());
                            } else {
                                version = text.trim().to_string();
                            }
                        },
                        _ => (),
                    }
                }

                let mut sections: Vec<(String, Vec<String>)> = Vec::new();
                let mut notes: Vec<String> = Vec::new();

                // h3 sections
                // and paragraphs without sections aka notes
                while let Some(event) = parser.next_if(|e| {
                    !matches!(
                        e,
                        &Event::Start(Tag::Heading {
                            level: HeadingLevel::H2,
                            ..
                        })
                    )
                }) {
                    match event {
                        // h3 section header
                        // starts a new section
                        Event::Start(Tag::Heading {
                            level: HeadingLevel::H3,
                            ..
                        }) => {
                            let mut section_name: Option<String> = None;
                            let mut section_lines: Vec<String> = Vec::new();

                            // h3 section header text
                            while let Some(event) = parser.next() {
                                match event {
                                    Event::End(TagEnd::Heading(HeadingLevel::H3)) => {
                                        break;
                                    },
                                    Event::Text(text) => {
                                        section_name = Some(text.trim().to_string());
                                    },
                                    _ => (),
                                }
                            }

                            // section list
                            while let Some(event) = parser.next_if(|e| {
                                !matches!(
                                    e,
                                    &Event::Start(Tag::Heading {
                                        level: HeadingLevel::H2,
                                        ..
                                    })
                                ) && !matches!(
                                    e,
                                    &Event::Start(Tag::Heading {
                                        level: HeadingLevel::H3,
                                        ..
                                    })
                                )
                            }) {
                                if let Event::Start(Tag::Item) = event {
                                    let mut item_text: String = String::new();

                                    while let Some(event) = parser.next() {
                                        match event {
                                            Event::End(TagEnd::Item) => break,
                                            Event::Text(text) => {
                                                item_text.push_str(&text);
                                            },
                                            Event::Code(text) => {
                                                item_text.push('"');
                                                item_text.push_str(&text);
                                                item_text.push('"');
                                            },
                                            Event::SoftBreak => {
                                                item_text.push(' ');
                                            },
                                            _ => (),
                                        }
                                    }
                                    section_lines.push(item_text);
                                }
                            }

                            // section done
                            // save if not empty
                            if let Some(section_name) =
                                section_name.filter(|_| !section_lines.is_empty())
                            {
                                sections.push((section_name, section_lines));
                            }
                        },
                        // paragraph without section aka note
                        Event::Start(Tag::Paragraph) => {
                            while let Some(event) = parser.next() {
                                match event {
                                    Event::End(TagEnd::Paragraph) => break,
                                    Event::Text(text) => {
                                        notes.push(text.to_string());
                                    },
                                    _ => (),
                                }
                            }
                        },
                        _ => (),
                    }
                }

                // version done
                // save if not empty
                if !sections.is_empty() || !notes.is_empty() {
                    versions.push(ChangelogVersion {
                        version,
                        date,
                        sections,
                        notes,
                    })
                }
            }
        }

        Ok(Some(ChangelogPanelComponent {
            etag,
            versions,
            display_count: 2,
        }))
    }

    /// Returns new Changelog in case remote one is newer
    async fn update_changelog(version: String, channel: Channel) -> Result<Option<Self>> {
        match net::query_etag(consts::CHANGELOG_URL.replace("{tag}", &channel.0)).await? {
            Some(remote_version) => {
                if version != remote_version {
                    debug!(
                        "Changelog version different (Local: {} Remote: {}), fetching...",
                        version, remote_version
                    );
                    Self::fetch(channel).await
                } else {
                    debug!("Changelog up-to-date.");
                    Ok(None)
                }
            },
            // We query the changelog in case there's no etag to be found
            // to make sure the player stays informed.
            None => {
                debug!("Changelog remote version missing, fetching...");
                Self::fetch(channel).await
            },
        }
    }

    fn cache_file() -> std::path::PathBuf {
        crate::fs::get_cache_path().join("changelog.ron")
    }

    pub async fn load_changelog() -> Result<Self> {
        Ok(from_str(
            &tokio::fs::read_to_string(&Self::cache_file()).await?,
        )?)
    }

    async fn save_changelog(self) {
        match to_string_pretty(&self, PrettyConfig::default()) {
            Ok(ron_string) => {
                if let Err(e) = tokio::fs::write(Self::cache_file(), ron_string).await {
                    tracing::warn!(?e, "Could not cache changelog");
                };
            },
            Err(e) => tracing::warn!(?e, "Could not serialize changelog for caching"),
        }
    }

    pub fn update(
        &mut self,
        msg: ChangelogPanelMessage,
    ) -> Option<Command<DefaultViewMessage>> {
        match msg {
            ChangelogPanelMessage::LoadChangelog(result, channel) => match result {
                Ok(changelog) => {
                    *self = changelog;
                    Some(Command::perform(
                        Self::update_changelog(self.etag.clone(), channel),
                        |update| {
                            DefaultViewMessage::ChangelogPanel(
                                ChangelogPanelMessage::UpdateChangelog(update),
                            )
                        },
                    ))
                },
                Err(e) => {
                    tracing::trace!(?e, "Failed to load changelog");
                    Some(Command::perform(Self::fetch(channel), |update| {
                        DefaultViewMessage::ChangelogPanel(
                            ChangelogPanelMessage::UpdateChangelog(update),
                        )
                    }))
                },
            },
            ChangelogPanelMessage::UpdateChangelog(result) => match result {
                Ok(Some(changelog)) => {
                    *self = changelog;
                    Some(Command::perform(Self::save_changelog(self.clone()), |_| {
                        DefaultViewMessage::ChangelogPanel(
                            ChangelogPanelMessage::SaveChangelog,
                        )
                    }))
                },
                Ok(None) => None,
                Err(e) => {
                    tracing::trace!("Failed to update changelog: {}", e);
                    None
                },
            },
            ChangelogPanelMessage::SaveChangelog => None,
            ChangelogPanelMessage::ScrollPositionChanged(pos) => {
                if pos > 0.9 && self.display_count < self.versions.len() {
                    self.display_count += 1;
                }
                None
            },
        }
    }

    pub fn view(&self) -> Element<'_, DefaultViewMessage> {
        let mut changelog = column![].spacing(10);

        for version in &mut self.versions.iter().take(self.display_count) {
            changelog = changelog.push(version.view());
        }

        let top_row = container(
            row![]
                .push(
                    container(Image::new(Handle::from_memory(CHANGELOG_ICON.to_vec())))
                        .height(Length::Fill)
                        .width(Length::Shrink)
                        .padding([0, 10, 0, 0])
                        .align_y(Vertical::Center),
                )
                .push(
                    container(
                        text("Latest Patch Notes")
                            .style(TextStyle::Dark)
                            .size(14)
                            .font(POPPINS_MEDIUM_FONT),
                    )
                    .padding([3, 0, 0, 0])
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_y(Vertical::Center),
                )
                .push(
                    container(
                        button(
                            row![]
                                .push(
                                    text("Recent Changes")
                                        .style(TextStyle::LightGrey)
                                        .size(10)
                                        .font(POPPINS_MEDIUM_FONT)
                                        .horizontal_alignment(Horizontal::Center),
                                )
                                .push(image(Handle::from_memory(
                                    UP_RIGHT_ARROW_ICON.to_vec(),
                                )))
                                .spacing(5)
                                .align_items(Alignment::Center),
                        )
                        .on_press(DefaultViewMessage::Interaction(Interaction::OpenURL(
                            GITLAB_MERGED_MR_URL.to_string(),
                        )))
                        .padding([4, 10, 0, 10])
                        .height(Length::Fixed(20.0))
                        .style(ButtonStyle::Browser(BrowserButtonStyle::Gitlab)),
                    )
                    .padding([0, 10, 0, 0])
                    .height(Length::Fill)
                    .align_y(Vertical::Center)
                    .width(Length::Shrink),
                )
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .align_y(Vertical::Center)
        .padding(10)
        .height(Length::Fixed(50.0))
        .style(ContainerStyle::ChangelogHeader);

        let col = column![].push(top_row).push(
            column![].push(
                container(
                    scrollable(changelog)
                        .on_scroll(|pos| {
                            DefaultViewMessage::ChangelogPanel(
                                ChangelogPanelMessage::ScrollPositionChanged(
                                    pos.relative_offset().y,
                                ),
                            )
                        })
                        .height(Length::Fill),
                )
                .height(Length::Fill)
                .width(Length::Fill)
                .style(ContainerStyle::Dark),
            ),
        );

        let changelog_container = container(col);
        changelog_container.into()
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogVersion {
    pub version: String,
    pub date: Option<String>,
    pub notes: Vec<String>,
    pub sections: Vec<(String, Vec<String>)>,
}

impl ChangelogVersion {
    pub fn view(&self) -> Element<'_, DefaultViewMessage> {
        let version_string = match &self.date {
            Some(date) => format!("v{} ({})", self.version, date),
            None => match self.version.as_str() {
                "Unreleased" => "Nightly".to_string(),
                _ => format!("v{}", self.version),
            },
        };

        let mut version = column![].spacing(10).push(
            column![]
                .push(
                    container(text(version_string).font(POPPINS_BOLD_FONT).size(20))
                        .padding([20, 0, 6, 33]),
                )
                .push(Rule::horizontal(8)),
        );

        for note in &self.notes {
            version = version.push(text(note).size(14));
        }

        for (section_name, section_lines) in &self.sections {
            let mut section_col = column![]
                .push(
                    text(section_name)
                        .size(16)
                        .line_height(LineHeight::Relative(2.0)),
                )
                .spacing(2);

            for line in section_lines {
                section_col = section_col.push(
                    container(
                        row![]
                            .push(
                                text(" •  ")
                                    .font(POPPINS_LIGHT_FONT)
                                    .size(12)
                                    .line_height(LineHeight::Absolute(16.into())),
                            )
                            .push(
                                text(line)
                                    .font(POPPINS_LIGHT_FONT)
                                    .size(12)
                                    .line_height(LineHeight::Absolute(16.into())),
                            ),
                    )
                    .padding([0, 0, 1, 10]),
                );
            }

            version = version.push(container(section_col).padding([0, 33]));
        }
        container(version).into()
    }
}
