mod app;
mod player;
#[path = "video_backend/gstreamer.rs"]
mod video_backend;

fn main() {
    app::run("opencut-player");
}
