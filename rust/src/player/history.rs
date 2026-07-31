use super::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_HISTORY_ITEMS: usize = 50;

#[derive(Deserialize, Serialize)]
struct HistorySettings {
    history_width: f32,
}

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct HistoryEntry {
    path: PathBuf,
    title: String,
    opened_at: u64,
    #[serde(default)]
    thumbnail: Option<PathBuf>,
}

#[derive(Default, Deserialize, Serialize)]
pub(super) struct HistoryData {
    items: Vec<HistoryEntry>,
}

impl HistoryData {
    pub(super) fn load() -> Self {
        let path = history_path();
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == ErrorKind::NotFound => return Self::default(),
            Err(error) => {
                eprintln!("Could not read {}: {error}", path.display());
                return Self::default();
            }
        };

        match serde_json::from_str::<Self>(&contents) {
            Ok(mut history) => {
                let mut changed = false;
                for entry in &mut history.items {
                    if entry.thumbnail.is_none() {
                        entry.thumbnail = Some(thumbnail_path(&entry.path));
                        changed = true;
                    }
                    if let Some(thumbnail) = &entry.thumbnail {
                        schedule_thumbnail(entry.path.clone(), thumbnail.clone());
                    }
                }
                if changed && let Err(error) = history.save() {
                    eprintln!("Could not update playback history: {error}");
                }
                history
            }
            Err(error) => {
                eprintln!("Could not parse {}: {error}", path.display());
                Self::default()
            }
        }
    }

    pub(super) fn record(&mut self, path: &Path, title: String) {
        let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let thumbnail = self
            .items
            .iter()
            .find(|entry| entry.path == path)
            .and_then(|entry| entry.thumbnail.clone())
            .unwrap_or_else(|| thumbnail_path(&path));
        self.items.retain(|entry| entry.path != path);
        self.items.insert(
            0,
            HistoryEntry {
                path: path.clone(),
                title,
                opened_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                thumbnail: Some(thumbnail.clone()),
            },
        );
        let removed = if self.items.len() > MAX_HISTORY_ITEMS {
            self.items.split_off(MAX_HISTORY_ITEMS)
        } else {
            Vec::new()
        };
        for entry in removed {
            remove_thumbnail(&entry);
        }

        if let Err(error) = self.save() {
            eprintln!("Could not save playback history: {error}");
        }
        schedule_thumbnail(path, thumbnail);
    }

    pub(super) fn remove(&mut self, path: &Path) {
        if let Some(index) = self.items.iter().position(|entry| entry.path == path) {
            let entry = self.items.remove(index);
            remove_thumbnail(&entry);
            if let Err(error) = self.save() {
                eprintln!("Could not save playback history: {error}");
            }
        }
    }

    fn save(&self) -> Result<(), String> {
        let path = history_path();
        if let Some(directory) = path.parent() {
            fs::create_dir_all(directory)
                .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|error| format!("could not serialize history: {error}"))?;
        fs::write(&path, format!("{json}\n"))
            .map_err(|error| format!("could not write {}: {error}", path.display()))
    }
}

pub(super) fn load_history_width() -> f32 {
    let path = settings_path();
    let Ok(contents) = fs::read_to_string(&path) else {
        return DEFAULT_HISTORY_WIDTH;
    };
    let Ok(settings) = serde_json::from_str::<HistorySettings>(&contents) else {
        eprintln!("Could not parse {}", path.display());
        return DEFAULT_HISTORY_WIDTH;
    };

    if settings.history_width.is_finite() {
        settings
            .history_width
            .clamp(MIN_HISTORY_WIDTH, MAX_HISTORY_WIDTH)
    } else {
        DEFAULT_HISTORY_WIDTH
    }
}

pub(super) fn save_history_width(width: f32) {
    let path = settings_path();
    let settings = HistorySettings {
        history_width: width.clamp(MIN_HISTORY_WIDTH, MAX_HISTORY_WIDTH),
    };
    let result = (|| -> Result<(), String> {
        if let Some(directory) = path.parent() {
            fs::create_dir_all(directory)
                .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
        }
        let json = serde_json::to_string_pretty(&settings)
            .map_err(|error| format!("could not serialize settings: {error}"))?;
        fs::write(&path, format!("{json}\n"))
            .map_err(|error| format!("could not write {}: {error}", path.display()))
    })();

    if let Err(error) = result {
        eprintln!("Could not save sidebar settings: {error}");
    }
}

fn schedule_thumbnail(video_path: PathBuf, thumbnail_path: PathBuf) {
    if thumbnail_path.is_file() || !video_path.is_file() {
        return;
    }

    std::thread::spawn(move || {
        if let Err(error) = generate_thumbnail(&video_path, &thumbnail_path) {
            eprintln!(
                "Could not create thumbnail for {}: {error}",
                video_path.display()
            );
        }
    });
}

fn generate_thumbnail(video_path: &Path, output_path: &Path) -> Result<(), String> {
    crate::video_backend::generate_thumbnail(video_path, output_path)
}

fn thumbnail_path(video_path: &Path) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    video_path.hash(&mut hasher);
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data/thumbnails")
        .join(format!("{:016x}.png", hasher.finish()))
}

fn remove_thumbnail(entry: &HistoryEntry) {
    if let Some(thumbnail) = &entry.thumbnail
        && let Err(error) = fs::remove_file(thumbnail)
        && error.kind() != ErrorKind::NotFound
    {
        eprintln!("Could not remove {}: {error}", thumbnail.display());
    }
}

impl Player {
    pub(super) fn history_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let entries = self
            .history
            .items
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let path = entry.path.clone();
                let remove_path = path.clone();
                let selected = self.current_media_path.as_ref() == Some(&path);
                let thumbnail = entry
                    .thumbnail
                    .as_ref()
                    .filter(|thumbnail| thumbnail.is_file())
                    .cloned();
                let preview = if let Some(thumbnail) = thumbnail {
                    img(thumbnail)
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                        .into_any_element()
                } else {
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child("▶")
                        .into_any_element()
                };
                let title = Path::new(&entry.title)
                    .file_stem()
                    .map(|title| title.to_string_lossy().into_owned())
                    .unwrap_or_else(|| entry.title.clone());
                let location = entry
                    .path
                    .parent()
                    .and_then(Path::file_name)
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Local file".to_string());

                div()
                    .id(("history-item", index))
                    .h(px(68.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_3()
                    .cursor(CursorStyle::PointingHand)
                    .rounded_lg()
                    .px_3()
                    .bg(if selected {
                        rgb(0x1d1d20)
                    } else {
                        rgb(BACKGROUND)
                    })
                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                    .child(
                        div()
                            .relative()
                            .w(px(68.0))
                            .h(px(42.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .overflow_hidden()
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(BORDER))
                            .bg(rgb(0x151518))
                            .text_color(rgb(0x55555d))
                            .child(preview),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_sm().text_ellipsis().child(title))
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(rgb(MUTED))
                                    .text_ellipsis()
                                    .child(format!("MP4 · {location}")),
                            ),
                    )
                    .child(
                        div()
                            .id(("remove-history-item", index))
                            .size_7()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor(CursorStyle::PointingHand)
                            .rounded_md()
                            .occlude()
                            .text_color(rgb(MUTED))
                            .hover(|style| style.bg(rgb(0x2a1a1c)).text_color(rgb(ERROR)))
                            .child("×")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.history.remove(&remove_path);
                                cx.notify();
                            })),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_path(path.clone());
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();

        div()
            .id("history-panel")
            .relative()
            .w(px(self.history_width))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(if self.is_resizing_history {
                rgb(ACCENT)
            } else {
                rgb(BORDER)
            })
            .group_hover("history-resize", |style| style.border_color(rgb(ACCENT)))
            .bg(rgb(0x0a0a0b))
            .child(
                div()
                    .h(px(HEADER_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_5()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(MUTED))
                            .child("HISTORY"),
                    )
                    .child(
                        div()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(rgb(0x55555d))
                            .child(self.history.items.len().to_string()),
                    ),
            )
            .child(
                div()
                    .id("history-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .when(entries.is_empty(), |this| {
                        this.child(
                            div()
                                .p_3()
                                .text_sm()
                                .text_color(rgb(MUTED))
                                .child("Opened MP4 files will appear here."),
                        )
                    })
                    .children(entries),
            )
            .child(
                div()
                    .id("history-resize-handle")
                    .absolute()
                    .top_0()
                    .right(px(-3.0))
                    .w(px(6.0))
                    .h_full()
                    .group("history-resize")
                    .cursor(CursorStyle::ResizeLeftRight)
                    .occlude()
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::begin_history_resize))
                    .on_mouse_move(cx.listener(Self::resize_history))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_history_resize))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_history_resize)),
            )
            .into_any_element()
    }
}

fn history_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/history.json")
}

fn settings_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/settings.json")
}
