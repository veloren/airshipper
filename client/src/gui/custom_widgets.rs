use crate::{assets::POPPINS_BOLD_FONT, gui::widget::*};
use iced::{
    Alignment, Length,
    widget::{container, horizontal_rule, row, text},
};

pub(crate) fn heading_with_rule<'a, T: 'a>(heading_text: &'a str) -> Element<'a, T> {
    container(
        row![]
            .align_items(Alignment::Center)
            .push(container(horizontal_rule(8)).width(Length::Fixed(13.0)))
            .push(
                container(text(heading_text).font(POPPINS_BOLD_FONT).size(16))
                    .padding([0, 7]),
            )
            .push(container(horizontal_rule(8)).width(Length::Fill)),
    )
    .into()
}
