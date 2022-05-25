use crate::config::color::OnagreColor;
use iced::Background;
use iced_style::scrollable::{Scrollbar, Scroller};

#[derive(Debug, PartialEq)]
pub struct ScrollerStyles {
    pub background: OnagreColor,
    pub border_color: OnagreColor,
    pub border_radius: f32,
    pub border_width: f32,
    pub scroller_color: OnagreColor,
    pub scroller_border_radius: f32,
    pub scroller_border_width: f32,
    pub scroller_border_color: OnagreColor,
    pub scrollbar_margin: u16,
    pub scrollbar_width: u16,
    pub scroller_width: u16,
}

impl Eq for ScrollerStyles {}

impl Default for ScrollerStyles {
    fn default() -> Self {
        ScrollerStyles {
            background: OnagreColor::DEFAULT_SCROLL,
            border_radius: 0.3,
            border_width: 0.0,
            border_color: OnagreColor::TRANSPARENT,
            scroller_color: OnagreColor::DEFAULT_SCROLLER,
            scroller_border_radius: 3.0,
            scroller_border_width: 0.0,
            scroller_border_color: OnagreColor::DEFAULT_BORDER,
            scrollbar_margin: 0,
            scrollbar_width: 4,
            scroller_width: 6,
        }
    }
}

impl iced::scrollable::StyleSheet for &ScrollerStyles {
    fn active(&self) -> Scrollbar {
        Scrollbar {
            background: Some(Background::Color(self.background.into())),
            border_radius: self.border_radius,
            border_width: self.border_width,
            border_color: self.border_color.into(),
            scroller: Scroller {
                color: self.scroller_color.into(),
                border_radius: self.scroller_border_radius,
                border_width: self.scroller_border_width,
                border_color: self.scroller_border_color.into(),
            },
        }
    }

    fn hovered(&self) -> Scrollbar {
        self.active()
    }
}