use std::borrow::Cow;
use gpui::{AssetSource, Result, SharedString};

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(match path {
            "icons/settings.svg"       => Some(Cow::Borrowed(include_bytes!("../assets/icons/settings.svg"))),
            "icons/star.svg"           => Some(Cow::Borrowed(include_bytes!("../assets/icons/star.svg"))),
            "icons/star-filled.svg"    => Some(Cow::Borrowed(include_bytes!("../assets/icons/star-filled.svg"))),
            "icons/pin-filled.svg"     => Some(Cow::Borrowed(include_bytes!("../assets/icons/pin-filled.svg"))),
            "icons/btn-shape.svg"      => Some(Cow::Borrowed(include_bytes!("../assets/icons/btn-shape.svg"))),
            "icons/pin.svg"            => Some(Cow::Borrowed(include_bytes!("../assets/icons/pin.svg"))),
            "icons/discord.svg"        => Some(Cow::Borrowed(include_bytes!("../assets/icons/discord.svg"))),
            "icons/harmoniya.svg"      => Some(Cow::Borrowed(include_bytes!("../assets/icons/harmoniya.svg"))),
            "icons/dots-vertical.svg"  => Some(Cow::Borrowed(include_bytes!("../assets/icons/dots-vertical.svg"))),
            "icons/newspaper.svg"      => Some(Cow::Borrowed(include_bytes!("../assets/icons/newspaper.svg"))),
            "icons/file-text.svg"      => Some(Cow::Borrowed(include_bytes!("../assets/icons/file-text.svg"))),
            "icons/power.svg"          => Some(Cow::Borrowed(include_bytes!("../assets/icons/power.svg"))),
            "icons/play.svg"           => Some(Cow::Borrowed(include_bytes!("../assets/icons/play.svg"))),
            "icons/arrow-left.svg"     => Some(Cow::Borrowed(include_bytes!("../assets/icons/arrow-left.svg"))),
            "icons/x.svg"              => Some(Cow::Borrowed(include_bytes!("../assets/icons/x.svg"))),
            "icons/shirt.svg"          => Some(Cow::Borrowed(include_bytes!("../assets/icons/shirt.svg"))),
            "icons/rocket.svg"         => Some(Cow::Borrowed(include_bytes!("../assets/icons/rocket.svg"))),
            "icons/log-out.svg"        => Some(Cow::Borrowed(include_bytes!("../assets/icons/log-out.svg"))),
            "icons/clock.svg"          => Some(Cow::Borrowed(include_bytes!("../assets/icons/clock.svg"))),
            "icons/arrow-up-right.svg" => Some(Cow::Borrowed(include_bytes!("../assets/icons/arrow-up-right.svg"))),
            _ => None,
        })
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![])
    }
}
