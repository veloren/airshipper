use crate::{ClientError, Result, fs, gui::views::default::DefaultViewMessage, net};
use futures_util::future::join_all;
use iced::{
    Task,
    widget::image::{Allocation, Handle, allocate},
};
use image::{ExtendedColorType, ImageFormat, imageops::FilterType};
use ron::{
    de::from_str,
    ser::{PrettyConfig, to_string_pretty},
};
use rss::Channel;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};

#[derive(Clone, Debug)]
pub enum RssFeedUpdateStatus {
    NoUpdateRequired,
    UpdateFailed(ClientError),
    Loaded(RssFeedData),
    Updated(RssFeedData),
    Saved,
}

#[derive(Clone, Debug)]
pub enum RssFeedComponentMessage {
    UpdateRssFeed(RssFeedUpdateStatus),
    ImageFetched {
        url: String,
        result: std::result::Result<Allocation, iced::widget::image::Error>,
    },
}

/// Allows a component to handle updates to an RSS feed that it owns
pub trait RssFeedComponent: 'static {
    const IMAGE_HEIGHT: u32;
    const NAME: &str;
    const FEED_URL: &str;

    /// Stores the feed against the component's own state
    fn store_feed(&mut self, rss_feed_data: RssFeedData);

    /// Returns the posts that the component has previously fetched from the RSS feed
    fn posts(&self) -> Vec<RssPost>;
    fn posts_mut(&mut self) -> Vec<&mut RssPost>;

    /// Returns the associated DefaultViewMessage for the given RssFeedComponentMessage
    fn rss_feed_message(message: RssFeedComponentMessage) -> DefaultViewMessage;

    /// An optional hook that is called after the RSS feed is updated
    fn after_rss_feed_updated(&mut self) {}

    fn handle_update(
        &mut self,
        msg: RssFeedComponentMessage,
    ) -> Option<Task<DefaultViewMessage>> {
        match msg {
            RssFeedComponentMessage::UpdateRssFeed(status) => match status {
                RssFeedUpdateStatus::Loaded(feed_data) => {
                    let etag = feed_data.etag.clone();
                    self.store_feed(feed_data);
                    Some(
                        Task::future(async {
                            let (status, task) = RssFeedData::update_feed(
                                Self::FEED_URL,
                                Self::NAME,
                                Self::IMAGE_HEIGHT,
                                etag,
                            )
                            .await;
                            (
                                Self::rss_feed_message(
                                    RssFeedComponentMessage::UpdateRssFeed(status),
                                ),
                                task.map(Self::rss_feed_message),
                            )
                        })
                        .then(|(message, task)| Task::batch([Task::done(message), task])),
                    )
                },
                RssFeedUpdateStatus::Updated(feed_data) => {
                    self.store_feed(feed_data.clone());
                    self.after_rss_feed_updated();

                    Some(Task::perform(feed_data.save_feed(Self::NAME), |_| {
                        Self::rss_feed_message(RssFeedComponentMessage::UpdateRssFeed(
                            RssFeedUpdateStatus::Saved,
                        ))
                    }))
                },
                RssFeedUpdateStatus::Saved => None,
                RssFeedUpdateStatus::NoUpdateRequired => {
                    // On application startup when there's been no rss update since the
                    // last run the posts will have been de-serialized  without their
                    // image data (which we don't store in the state ron file) so we need
                    // to attempt to fetch the image for them, which
                    // should reside in the cache if it hasn't been
                    // cleared.
                    let posts = self.posts();
                    let commands: Vec<Task<DefaultViewMessage>> = posts
                        .iter()
                        .filter_map(|post| {
                            // Filter out posts that we already have the image for, or
                            // don't have an image URL
                            if post.image.is_some() || post.image_url.is_none() {
                                return None;
                            }
                            let url = post.image_url.as_ref().unwrap().to_owned();
                            let image_cache_name = post.image_cache_name();
                            Some(
                                Task::future({
                                    let url = url.clone();
                                    async move {
                                        RssPost::fetch_image(
                                            url.as_str(),
                                            Self::NAME,
                                            image_cache_name,
                                            Self::IMAGE_HEIGHT,
                                        )
                                        .await
                                    }
                                })
                                .map(Result::ok)
                                .and_then(allocate)
                                .then(move |result| {
                                    Task::done(Self::rss_feed_message(
                                        RssFeedComponentMessage::ImageFetched {
                                            url: url.clone(),
                                            result,
                                        },
                                    ))
                                }),
                            )
                        })
                        .collect();

                    self.after_rss_feed_updated();

                    debug!("Fetching images for {} cached blog posts", commands.len());
                    Some(Task::batch(commands))
                },
                RssFeedUpdateStatus::UpdateFailed(e) => {
                    error!(?e, "Failed to fetch RSS feed");
                    None
                },
            },
            RssFeedComponentMessage::ImageFetched { result, url } => {
                if let Ok(allocation) = result.map_err(|e| {
                    tracing::error!("Failed to allocate rss feed image memory: {:?}", e)
                }) && let Some(post) = self
                    .posts_mut()
                    .iter_mut()
                    .filter(|post| post.image_url.is_some())
                    .find(|post| post.image_url.as_ref().unwrap() == &url)
                {
                    post.image = Some(allocation);
                }

                None
            },
        }
    }
}

/// Represents the parsed data downloaded from an RSS feed URL, comprised of individual
/// posts and an etag used to check if the RSS feed has been updated since it was last
/// queried
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct RssFeedData {
    pub posts: Vec<RssPost>,
    pub etag: String,
}

impl RssFeedData {
    async fn update_feed(
        feed_url: &str,
        name: &str,
        height: u32,
        local_version: String,
    ) -> (RssFeedUpdateStatus, Task<RssFeedComponentMessage>) {
        let fetch = move |local_version: String| async move {
            match net::query_etag(feed_url).await? {
                Some(remote_version) => {
                    if local_version != remote_version {
                        debug!(
                            ?feed_url,
                            "Local version {} does not match remote version {}, \
                             fetching feed",
                            local_version,
                            remote_version
                        );
                        let (feed, task) = Self::fetch(feed_url, name, height).await?;
                        Ok((RssFeedUpdateStatus::Updated(feed), task))
                    } else {
                        debug!(?feed_url, "RSS feed up-to-date.");
                        Ok((RssFeedUpdateStatus::NoUpdateRequired, Task::none()))
                    }
                },
                // If no etag was found, perform a full update
                None => {
                    warn!(
                        ?feed_url,
                        "No etag found for RSS feed, assuming an update is required."
                    );
                    let (feed, task) = Self::fetch(feed_url, name, height).await?;
                    Ok((RssFeedUpdateStatus::Updated(feed), task))
                },
            }
        };

        fetch(local_version)
            .await
            .unwrap_or_else(|e| (RssFeedUpdateStatus::UpdateFailed(e), Task::none()))
    }

    fn cache_file(name: &str) -> std::path::PathBuf {
        crate::fs::get_cache_path().join(format!("{name}.ron"))
    }

    pub async fn load_feed(
        feed_url: &str,
        name: &str,
        height: u32,
    ) -> (RssFeedUpdateStatus, Task<RssFeedComponentMessage>) {
        match tokio::fs::read_to_string(&Self::cache_file(name)).await {
            Ok(string) => match from_str(&string) {
                Ok(feed_data) => {
                    return (RssFeedUpdateStatus::Loaded(feed_data), Task::none());
                },
                Err(e) => tracing::trace!(
                    ?e,
                    "Failed to deserialize cached feed data for feed: {}",
                    name
                ),
            },
            Err(e) => {
                tracing::trace!(?e, "Failed to read cached feed data for feed: {}", name)
            },
        }

        match Self::fetch(feed_url, name, height).await {
            Ok((feed_data, task)) => (RssFeedUpdateStatus::Updated(feed_data), task),
            Err(e) => (RssFeedUpdateStatus::UpdateFailed(e), Task::none()),
        }
    }

    async fn save_feed(self, name: &str) {
        match to_string_pretty(&self, PrettyConfig::default()) {
            Ok(ron_string) => {
                if let Err(e) = tokio::fs::write(Self::cache_file(name), ron_string).await
                {
                    tracing::warn!(?e, "Could not cache feed data for feed: {}", name);
                };
            },
            Err(e) => tracing::warn!(
                ?e,
                "Could not serialize feed data for caching of feed: {}",
                name
            ),
        }
    }

    async fn fetch(
        feed_url: &str,
        name: &str,
        height: u32,
    ) -> Result<(RssFeedData, Task<RssFeedComponentMessage>)> {
        use std::io::BufReader;

        let feed_response = net::query(feed_url).await?;
        let etag = net::get_etag(&feed_response);
        let feed = Channel::read_from(BufReader::new(&feed_response.bytes().await?[..]))?;

        let posts = feed
            .items
            .into_iter()
            // TODO: Currently we want 15 blog posts and 15 community showcase posts - if this is ever not the case
            // then this number will need parameterising.
            .take(15)
            .map(RssPost::from)
            .collect::<Vec<_>>();

        let tasks = join_all(posts.iter().map(|post| async {
            if let Some(url) = &post.image_url
                && let Ok(handle) =
                    RssPost::fetch_image(url, name, post.image_cache_name(), height).await
            {
                let url = url.clone();
                allocate(handle).then(move |result| {
                    let url = url.clone();
                    Task::done(RssFeedComponentMessage::ImageFetched { url, result })
                })
            } else {
                Task::none()
            }
        }))
        .await;

        if let Ok(dir) = std::fs::read_dir(RssPost::cache_base_path(name)) {
            for file in dir.flatten() {
                if let Ok(file_name) = file.file_name().into_string()
                    && !posts.iter().any(|i| i.image_cache_name() == file_name)
                {
                    std::fs::remove_file(file.path())?;
                }
            }
        }

        Ok((RssFeedData { posts, etag }, Task::batch(tasks)))
    }
}

/// An individual post parsed from an RSS feed
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct RssPost {
    pub title: String,
    pub description: String,
    pub button_url: String,
    pub image_url: Option<String>,
    #[serde(skip)]
    pub image: Option<Allocation>,
}

impl RssPost {
    /// Attempts to fetch an image for a given URL, retrieving it from the RSS image
    /// cache if possible, otherwise attempting to download it from the URL.
    /// Images are resized before being cached, (as well as after being retrieved
    /// from the cache since full-sized images may have been cached in older versions)
    /// to save memory and improve performance.
    /// Images are returned raw in order to circumvent a performance issue in the
    /// currently utilized version (0.12) of iced where images scrolled out of view
    /// are constantly being re-decoded, significantly lagging the UI.
    pub async fn fetch_image(
        url: &str,
        feed_name: &str,
        image_cache_name: String,
        height: u32,
    ) -> Result<Handle> {
        let cache_base_path = Self::cache_base_path(feed_name);
        std::fs::create_dir_all(&cache_base_path)?;
        let image_cache_path = cache_base_path.join(image_cache_name);

        if let Ok(cached_bytes) = std::fs::read(&image_cache_path) {
            // Found the image cached locally so use it
            debug!(
                "Retrieved cached image for URL {} from path {}",
                url,
                image_cache_path.to_string_lossy()
            );
            let image = image::load_from_memory(&cached_bytes)?.into_rgba8();
            return Ok(Handle::from_rgba(
                image.width(),
                image.height(),
                image.into_raw(),
            ));
        }

        match crate::net::client::WEB_CLIENT.get(url).send().await {
            Ok(response) => match response.bytes().await {
                Ok(bytes) => match image::load_from_memory(&bytes) {
                    Ok(image) => {
                        // Image successfully downloaded, write it to the cache before
                        // returning it
                        debug!(
                            "Caching image from URL {} with path {}",
                            url,
                            image_cache_path.to_string_lossy()
                        );
                        // Decode the image and resize it to the specified height,
                        // preserving aspect ratio. Works best if
                        // said aspect ratio is 16:9 or wider.
                        let rgba8 =
                            image.resize(1000, height, FilterType::Nearest).into_rgba8();
                        image::save_buffer_with_format(
                            &image_cache_path,
                            rgba8.as_raw(),
                            rgba8.width(),
                            rgba8.height(),
                            ExtendedColorType::Rgba8,
                            ImageFormat::Png,
                        )?;
                        Ok(Handle::from_rgba(
                            rgba8.width(),
                            rgba8.height(),
                            rgba8.into_raw(),
                        ))
                    },
                    Err(e) => {
                        error!(?e, ?url, "Failed to decode image");
                        Err(e.into())
                    },
                },
                Err(e) => {
                    error!("Failed to fetch bytes of RSS image from URL {}", url);
                    Err(e.into())
                },
            },
            Err(e) => {
                error!("Failed to download RSS image from URL {}", url);
                Err(e.into())
            },
        }
    }

    fn cache_base_path(feed_name: &str) -> std::path::PathBuf {
        fs::get_cache_path().join(format!("{}_images", feed_name))
    }

    fn image_cache_name(&self) -> String {
        for item in self.button_url.split('/').rev() {
            if !item.is_empty() {
                return item.to_string();
            }
        }
        self.title.clone()
    }

    fn process_description(desc: Option<String>) -> String {
        match desc {
            Some(desc) => {
                let wrapped_html = html2text::from_read(desc.as_bytes(), 400);
                if let Ok(html) = wrapped_html {
                    let stripped_html = html
                        .lines()
                        .take(3)
                        .filter(|x| !x.contains("[banner]"))
                        .fold(String::new(), |mut output, b| {
                            use std::fmt::Write;
                            let _ = writeln!(output, "{b}");
                            output
                        });
                    strip_markdown::strip_markdown(&stripped_html)
                        .trim_end()
                        .to_owned()
                } else {
                    "HTML parsing failed.".into()
                }
            },
            None => "No description found.".into(),
        }
    }
}

impl From<rss::Item> for RssPost {
    fn from(item: rss::Item) -> Self {
        let mut post = RssPost {
            title: item.title.unwrap_or_else(|| "Missing title".into()),
            description: Self::process_description(item.description),
            button_url: item.link.unwrap_or_else(|| "https://veloren.net".into()),
            image_url: None,
            image: None,
        };

        // If the RSS item has an enclosure (attached media), store the URL against
        // the post for display in the RSS feed.
        if let Some(enclosure) = item.enclosure.as_ref() {
            post.image_url = Some(enclosure.url.clone());
        }

        post
    }
}
