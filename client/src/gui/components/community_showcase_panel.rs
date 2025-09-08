use crate::{
    consts,
    gui::{
        custom_widgets::heading_with_rule,
        rss_feed::{
            RssFeedComponent, RssFeedComponentMessage, RssFeedData, RssFeedUpdateStatus,
            RssPost,
        },
        style::{button::ButtonStyle, container::ContainerStyle},
        views::default::{DefaultViewMessage, Interaction},
        widget::*,
    },
};
use iced::{
    Command, ContentFit, Length,
    alignment::{Horizontal, Vertical},
    widget::{
        Image, Space, button, column, container, row, text, tooltip, tooltip::Position,
    },
};
use rand::{prelude::SliceRandom, rng};
use serde::{Deserialize, Serialize};
use std::cmp::{max, min};

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CommunityShowcaseComponent {
    posts: Vec<CommunityPost>,
    etag: String,
    offset: usize,
}

#[derive(Clone, Debug)]
pub enum PostOffsetChange {
    Increment,
    Decrement,
}

#[derive(Clone, Debug)]
pub enum CommunityShowcasePanelMessage {
    RssUpdate(RssFeedComponentMessage),
    PostOffsetChange(PostOffsetChange),
}

impl RssFeedComponent for CommunityShowcaseComponent {
    const IMAGE_HEIGHT: u32 = 180;
    const NAME: &str = "community_showcase";
    const FEED_URL: &str = consts::COMMUNITY_SHOWCASE_URL;

    fn store_feed(&mut self, rss_feed: RssFeedData) {
        self.posts = rss_feed
            .posts
            .into_iter()
            .map(|rss_post| CommunityPost { rss_post })
            .collect();

        self.etag = rss_feed.etag;
    }

    fn posts(&self) -> Vec<RssPost> {
        self.posts.iter().map(|x| x.rss_post.clone()).collect()
    }

    fn posts_mut(&mut self) -> Vec<&mut RssPost> {
        self.posts.iter_mut().map(|x| &mut x.rss_post).collect()
    }

    fn rss_feed_message(message: RssFeedComponentMessage) -> DefaultViewMessage {
        DefaultViewMessage::CommunityShowcasePanel(
            CommunityShowcasePanelMessage::RssUpdate(message),
        )
    }

    fn after_rss_feed_updated(&mut self) {
        // Shuffle Community Showcase posts each time they're loaded so that users
        // see different posts even if they never click the next/prev buttons.
        self.posts.shuffle(&mut rng());
    }
}

impl CommunityShowcaseComponent {
    // 16:9 Aspect ratio
    const IMAGE_WIDTH: u32 = 320;

    pub(crate) async fn load_community_posts() -> RssFeedUpdateStatus {
        RssFeedData::load_feed(Self::FEED_URL, Self::NAME, Self::IMAGE_HEIGHT).await
    }

    pub fn update(
        &mut self,
        msg: CommunityShowcasePanelMessage,
    ) -> Option<Command<DefaultViewMessage>> {
        match msg {
            CommunityShowcasePanelMessage::RssUpdate(rss_msg) => {
                self.handle_update(rss_msg)
            },
            CommunityShowcasePanelMessage::PostOffsetChange(post_offset_change) => {
                match post_offset_change {
                    PostOffsetChange::Increment => {
                        self.offset = min(self.offset + 1, self.posts.len() - 1);
                    },
                    PostOffsetChange::Decrement => {
                        self.offset = (self.offset - 1).clamp(0, self.posts.len() - 1)
                    },
                };

                None
            },
        }
    }

    pub fn view(&self) -> Element<DefaultViewMessage> {
        let current_post = if let Some(post) = self.posts.get(self.offset) {
            container(post.view()).width(Length::Fill)
        } else {
            container(text("Nothing to show"))
        };

        let prev_button = button(text("<< Prev").size(14))
            .style(ButtonStyle::NextPrev)
            .width(Length::Shrink)
            .on_press(DefaultViewMessage::CommunityShowcasePanel(
                CommunityShowcasePanelMessage::PostOffsetChange(
                    PostOffsetChange::Decrement,
                ),
            ));

        let next_button = button(text("Next >>").size(14))
            .style(ButtonStyle::NextPrev)
            .width(Length::Shrink)
            .on_press(DefaultViewMessage::CommunityShowcasePanel(
                CommunityShowcasePanelMessage::PostOffsetChange(
                    PostOffsetChange::Increment,
                ),
            ));

        let button_row = if self.offset == 0 {
            row![]
                .push(Space::with_width(Length::Fill))
                .push(next_button)
        } else if self.offset == max(self.posts.len(), 1) - 1 {
            row![].push(prev_button)
        } else {
            row![]
                .push(prev_button)
                .push(Space::with_width(Length::Fill))
                .push(next_button)
        };

        column![]
            .push(heading_with_rule("Community Showcase"))
            .push(
                container(column![].push(current_post).push(button_row))
                    .width(Length::Fill)
                    .padding([10, 20]),
            )
            .into()
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct CommunityPost {
    pub rss_post: RssPost,
}

impl CommunityPost {
    pub(crate) fn view(&self) -> Element<DefaultViewMessage> {
        let post = &self.rss_post;

        let image_container = if let Some(handle) = &post.image {
            container(
                tooltip(
                    container(
                        Image::new(handle.clone())
                            .content_fit(ContentFit::Cover)
                            .height(Length::Fixed(
                                CommunityShowcaseComponent::IMAGE_HEIGHT as f32,
                            ))
                            .width(Length::Fixed(
                                CommunityShowcaseComponent::IMAGE_WIDTH as f32,
                            )),
                    ),
                    text(&post.title).size(14),
                    Position::Right,
                )
                .style(ContainerStyle::Tooltip)
                .gap(5),
            )
        } else {
            container(text("Loading..."))
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center)
                .style(ContainerStyle::LoadingBlogPost)
                .height(Length::Fixed(
                    CommunityShowcaseComponent::IMAGE_HEIGHT as f32,
                ))
                .width(Length::Fixed(
                    CommunityShowcaseComponent::IMAGE_WIDTH as f32,
                ))
        };
        button(image_container)
            .style(ButtonStyle::Transparent)
            .on_press(DefaultViewMessage::Interaction(Interaction::OpenURL(
                post.button_url.clone(),
            )))
            .width(Length::Fixed(
                CommunityShowcaseComponent::IMAGE_WIDTH as f32,
            ))
            .into()
    }
}
