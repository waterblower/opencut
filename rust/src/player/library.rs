use super::*;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_HISTORY_ITEMS: usize = 50;

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct HistoryEntry {
    path: PathBuf,
    title: String,
    opened_at: u64,
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

        match serde_json::from_str(&contents) {
            Ok(history) => history,
            Err(error) => {
                eprintln!("Could not parse {}: {error}", path.display());
                Self::default()
            }
        }
    }

    pub(super) fn record(&mut self, path: &Path, title: String) {
        let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.items.retain(|entry| entry.path != path);
        self.items.insert(
            0,
            HistoryEntry {
                path,
                title,
                opened_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            },
        );
        self.items.truncate(MAX_HISTORY_ITEMS);

        if let Err(error) = self.save() {
            eprintln!("Could not save playback history: {error}");
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

impl Player {
    pub(super) fn library_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let entries = self
            .history
            .items
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let path = entry.path.clone();
                let selected = self.current_media_path.as_ref() == Some(&path);
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
                            .child("▶")
                            .when(selected, |this| {
                                this.child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .bottom_0()
                                        .h(px(3.0))
                                        .w_full()
                                        .bg(rgb(ACCENT)),
                                )
                            }),
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
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_path(path.clone());
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();

        div()
            .id("library-panel")
            .w(px(LIBRARY_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(rgb(BORDER))
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
                    .id("library-history-list")
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
            .into_any_element()
    }
}

fn history_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/history.json")
}
