use crate::gui::{
    style,
    views::default::{DefaultViewMessage, Interaction},
    widget::*,
};
use iced::{
    Length, Padding,
    alignment::Vertical,
    widget::{Image, button, column, container, row, text, text::Shaping},
};

#[derive(Clone, Default, Debug)]
pub struct LogoPanelComponent {}

impl LogoPanelComponent {
    pub fn view(&self) -> Element<'_, DefaultViewMessage> {
        let col = column![].push(icon::veloren_logo()).push(
            container(
                column![]
                    .push(link_widget(
                        icon::book(),
                        "https://book.veloren.net/",
                        "Game Manual",
                    ))
                    .push(link_widget(
                        icon::chat(),
                        "https://veloren.net/joinus/",
                        "Community",
                    ))
                    .push(link_widget(
                        icon::user(),
                        "https://veloren.net/account/",
                        "Create Account",
                    ))
                    .push(link_widget(
                        icon::heart(),
                        "https://opencollective.com/veloren/",
                        "Donate",
                    )),
            )
            .padding(Padding::ZERO.top(40)),
        );

        let container: Container<'_, DefaultViewMessage> = container(col).padding(20);
        container.into()
    }
}

fn link_widget<'a>(
    image: Option<Image>,
    url: &'a str,
    link_text: &'a str,
) -> Element<'a, DefaultViewMessage> {
    container(
        button(
            row![]
                .align_y(Vertical::Center)
                .push(
                    container(image.map(|image| {
                        image.height(Length::Fixed(24.0)).width(Length::Fixed(24.0))
                    }))
                    .align_y(Vertical::Center),
                )
                .push(
                    container(text(link_text).size(14).shaping(Shaping::Advanced))
                        .align_y(Vertical::Center),
                )
                .push(container(icon::up_right_arrow()).align_y(Vertical::Center))
                .spacing(10),
        )
        .padding(5)
        .on_press(DefaultViewMessage::Interaction(Interaction::OpenURL(
            url.to_string(),
        )))
        .style(style::button::transparent),
    )
    .height(Length::Shrink)
    .into()
}
