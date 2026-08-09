#[path = "../app.rs"]
mod app;
#[path = "../playback_view.rs"]
mod playback_view;
#[path = "mod.rs"]
mod player;
#[path = "../video.rs"]
mod video;

fn main() {
    app::run("opencut-player");
}
