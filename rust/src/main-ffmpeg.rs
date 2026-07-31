mod app;
mod player;
#[path = "video_backend/ffmpeg.rs"]
mod video_backend;

fn main() {
    app::run("opencut-player-ffmpeg");
}
