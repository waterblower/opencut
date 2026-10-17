use gpui::{AssetSource, SharedString};
use std::borrow::Cow;

#[derive(Clone, Copy)]
pub enum IconName {
    AlertTriangle,
    App,
    Eye,
    Folder,
    Lock,
    Mute,
    PickerGlow,
    Pointer,
    Spinner,
    Trash,
    Unmute,
    Warning,
}

// Public APIs only
impl IconName {
    pub(crate) const fn path(self) -> &'static str {
        match self {
            Self::AlertTriangle => "icons/alert-triangle.svg",
            Self::App => "icons/app-icon.svg",
            Self::Eye => "icons/eye.svg",
            Self::Folder => "icons/folder.svg",
            Self::Lock => "icons/lock.svg",
            Self::Mute => "icons/mute.svg",
            Self::PickerGlow => "icons/picker-glow.svg",
            Self::Pointer => "icons/pointer.svg",
            Self::Spinner => "icons/spinner.svg",
            Self::Trash => "icons/trash.svg",
            Self::Unmute => "icons/unmute.svg",
            Self::Warning => "icons/warning.svg",
        }
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

// Private APIs only
impl IconName {
    const ALL: [Self; 12] = [
        Self::AlertTriangle,
        Self::App,
        Self::Eye,
        Self::Folder,
        Self::Lock,
        Self::Mute,
        Self::PickerGlow,
        Self::Pointer,
        Self::Spinner,
        Self::Trash,
        Self::Unmute,
        Self::Warning,
    ];

    const fn bytes(self) -> &'static [u8] {
        match self {
            Self::AlertTriangle => include_bytes!("../icons/alert-triangle.svg"),
            Self::App => include_bytes!("../icons/app-icon.svg"),
            Self::Eye => include_bytes!("../icons/eye.svg"),
            Self::Folder => include_bytes!("../icons/folder.svg"),
            Self::Lock => include_bytes!("../icons/lock.svg"),
            Self::Mute => include_bytes!("../icons/mute.svg"),
            Self::PickerGlow => include_bytes!("../icons/picker-glow.svg"),
            Self::Pointer => include_bytes!("../icons/pointer.svg"),
            Self::Spinner => include_bytes!("../icons/spinner.svg"),
            Self::Trash => include_bytes!("../icons/trash.svg"),
            Self::Unmute => include_bytes!("../icons/unmute.svg"),
            Self::Warning => include_bytes!("../icons/warning.svg"),
        }
    }

    fn from_path(path: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|icon| icon.path() == path)
    }
}
