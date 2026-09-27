use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

#[derive(Clone, Copy)]
pub enum IconName {
    Eye,
    Lock,
    Mute,
    Trash,
    Unmute,
}

impl IconName {
    const ALL: [Self; 5] = [Self::Eye, Self::Lock, Self::Mute, Self::Trash, Self::Unmute];

    pub(crate) const fn path(self) -> &'static str {
        match self {
            Self::Eye => "icons/eye.svg",
            Self::Lock => "icons/lock.svg",
            Self::Mute => "icons/mute.svg",
            Self::Trash => "icons/trash.svg",
            Self::Unmute => "icons/unmute.svg",
        }
    }

    const fn bytes(self) -> &'static [u8] {
        match self {
            Self::Eye => include_bytes!("../icons/eye.svg"),
            Self::Lock => include_bytes!("../icons/lock.svg"),
            Self::Mute => include_bytes!("../icons/mute.svg"),
            Self::Trash => include_bytes!("../icons/trash.svg"),
            Self::Unmute => include_bytes!("../icons/unmute.svg"),
        }
    }

    fn from_path(path: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|icon| icon.path() == path)
    }
}

pub struct EditorAssets;

impl AssetSource for EditorAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let Some(icon) = IconName::from_path(path) else {
            return Ok(None);
        };
        Ok(Some(Cow::Borrowed(icon.bytes())))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(match path {
            "icons" => IconName::ALL
                .into_iter()
                .map(|icon| icon.path().trim_start_matches("icons/").into())
                .collect(),
            _ => Vec::new(),
        })
    }
}
