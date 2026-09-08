use super::*;
use std::{
    future::Future,
    path::PathBuf,
    pin::pin,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Poll, Wake, Waker},
};

#[test]
fn opens_paused_with_owned_frame_and_metadata() -> Result<()> {
    let fixture = Fixture::new("0")?;
    let mut backend = VideoBackend::open_sync(&fixture.0)?;
    assert!(backend.paused());
    assert_eq!(backend.position(), Duration::ZERO);
    assert_eq!(backend.frame_size(), (66, 34));
    assert_eq!(backend.framerate(), Some(25.0));
    assert_eq!(backend.duration(), Duration::from_millis(400));
    let initial = backend.get_current_frame()?;
    assert_eq!(initial.timestamp, Duration::ZERO);
    assert_eq!(initial.image.format(), ffmpeg_next::format::Pixel::YUV420P);
    assert!(initial.image.stride(0) > 66);
    assert!((75..90).contains(&initial.image.data(0)[0]));
    thread::sleep(Duration::from_millis(60));
    assert!(Arc::ptr_eq(&initial, &backend.get_current_frame()?));
    backend.set_volume(0.4)?;
    backend.set_muted(true)?;
    assert_eq!(backend.volume(), 0.0);
    backend.set_muted(false)?;
    assert_eq!(backend.volume(), 0.4);
    backend.set_volume(2.0)?;
    assert_eq!(backend.volume(), 1.0);
    assert!(backend.set_volume(f64::NAN).is_err());
    assert!(backend.set_volume(f64::INFINITY).is_err());
    backend.seek_sync(Duration::from_millis(205))?;
    assert_eq!(
        backend.get_current_frame()?.timestamp,
        Duration::from_millis(200)
    );
    assert_eq!(backend.position(), Duration::from_millis(205));
    assert_eq!(initial.timestamp, Duration::ZERO);
    Ok(())
}

#[test]
fn playback_advances_without_frame_reads_and_drains_b_frames() -> Result<()> {
    let fixture = Fixture::new("0")?;
    let mut backend = VideoBackend::open_sync(&fixture.0)?;
    backend.set_paused(false)?;
    // No getter or application-driven update runs during playback.
    thread::sleep(Duration::from_millis(650));
    assert!(backend.ended());
    assert!(backend.paused());
    assert_eq!(
        backend.get_current_frame()?.timestamp,
        Duration::from_millis(360)
    );
    assert_eq!(backend.position(), Duration::from_millis(400));
    backend.seek_sync(Duration::ZERO)?;
    assert!(!backend.ended());
    assert_eq!(backend.get_current_frame()?.timestamp, Duration::ZERO);
    Ok(())
}

#[test]
fn seeks_both_directions_including_delayed_frames_and_boundaries() -> Result<()> {
    let fixture = Fixture::new("0")?;
    let mut backend = block_on(VideoBackend::open(&fixture.0))?;
    for millis in [360, 320, 280, 240, 200, 160, 120, 80, 40, 0, 280, 120] {
        block_on(backend.seek(Duration::from_millis(millis)))?;
        assert!(backend.paused());
        assert_eq!(backend.position(), Duration::from_millis(millis));
        assert_eq!(
            backend.get_current_frame()?.timestamp,
            Duration::from_millis(millis)
        );
    }
    backend.seek_sync(Duration::from_secs(100))?;
    assert_eq!(
        backend.get_current_frame()?.timestamp,
        Duration::from_millis(360)
    );
    assert_eq!(backend.position(), Duration::from_millis(360));
    backend.set_paused(false)?;
    backend.seek_sync(Duration::from_millis(80))?;
    assert!(!backend.paused());
    backend.set_paused(true)?;
    let position = backend.position();
    thread::sleep(Duration::from_millis(50));
    assert_eq!(backend.position(), position);
    Ok(())
}

#[test]
fn nonzero_stream_origin_is_normalized() -> Result<()> {
    let fixture = Fixture::new("5")?;
    let mut backend = VideoBackend::open_sync(&fixture.0)?;
    assert_eq!(backend.get_current_frame()?.timestamp, Duration::ZERO);
    backend.seek_sync(Duration::from_millis(160))?;
    assert_eq!(
        backend.get_current_frame()?.timestamp,
        Duration::from_millis(160)
    );
    Ok(())
}

#[test]
fn async_seek_yields_until_acknowledged_and_canceled_wait_can_be_superseded() -> Result<()> {
    let (commands, requests) = mpsc::channel();
    let mut backend = VideoBackend {
        shared: Arc::new(Mutex::new(State::new())),
        commands,
        workers: Vec::new(),
    };
    let mut context = Context::from_waker(Waker::noop());
    let first;
    {
        let mut future = pin!(backend.seek(Duration::from_secs(1)));
        assert!(future.as_mut().poll(&mut context).is_pending());
        first = requests.try_recv().context(format!(
            "Receiving first test seek at {}:{}",
            file!(),
            line!()
        ))?;
    }
    let mut future = pin!(backend.seek(Duration::from_secs(2)));
    assert!(future.as_mut().poll(&mut context).is_pending());
    let second = requests.try_recv().context(format!(
        "Receiving second test seek at {}:{}",
        file!(),
        line!()
    ))?;
    assert!(second.generation > first.generation);
    assert!(first.reply.try_send(Ok(())).is_err());
    second.reply.try_send(Ok(())).context(format!(
        "Acknowledging test seek at {}:{}",
        file!(),
        line!()
    ))?;
    assert!(matches!(
        future.as_mut().poll(&mut context),
        Poll::Ready(Ok(()))
    ));
    Ok(())
}

#[test]
fn seek_completion_does_not_borrow_backend_and_publishes_paused_preview() -> Result<()> {
    let fixture = Fixture::new("0")?;
    let mut backend = VideoBackend::open_sync(&fixture.0)?;
    for target in [320, 200, 80] {
        let completion = backend.seek(Duration::from_millis(target));
        // These accesses must remain possible while the seek future is alive:
        // GPUI renders snapshots and handles controls before completion.
        assert!(backend.paused());
        let _snapshot = backend.get_current_frame()?;
        backend.set_muted(true)?;
        block_on(completion)?;
        assert_eq!(
            backend.get_current_frame()?.timestamp,
            Duration::from_millis(target)
        );
        assert!(backend.paused());
    }
    // Releasing a playing scrub changes the resume intent during the seek.
    let completion = backend.seek(Duration::from_millis(200));
    backend.set_paused(false)?;
    block_on(completion)?;
    assert!(!backend.paused());
    Ok(())
}

#[test]
fn repeated_paused_seek_reuses_the_completed_frame() -> Result<()> {
    let fixture = Fixture::new("0")?;
    let mut backend = VideoBackend::open_sync(&fixture.0)?;
    backend.seek_sync(Duration::from_millis(205))?;
    let frame = backend.get_current_frame()?;
    let generation = lock(&backend.shared).generation;
    backend.seek_sync(Duration::from_millis(205))?;
    assert!(Arc::ptr_eq(&frame, &backend.get_current_frame()?));
    assert_eq!(lock(&backend.shared).generation, generation);
    assert_eq!(backend.position(), Duration::from_millis(205));
    Ok(())
}

#[test]
#[ignore = "set VIDEO2_BENCH_PATH to a local video to measure synchronous seeks"]
fn synchronous_seek_latency() -> Result<()> {
    let path = std::env::var_os("VIDEO2_BENCH_PATH").context(format!(
        "Set VIDEO2_BENCH_PATH at {}:{}",
        file!(),
        line!()
    ))?;
    let mut backend = VideoBackend::open_sync(Path::new(&path))?;
    for fraction in [0.25, 0.25, 0.1, 0.1, 0.75, 0.75, 0.4, 0.41, 0.42] {
        backend.seek_sync(backend.duration().mul_f64(fraction))?;
    }
    Ok(())
}

#[test]
fn drop_interrupts_a_full_paused_video_queue() -> Result<()> {
    let fixture = Fixture::new("0")?;
    let backend = VideoBackend::open_sync(&fixture.0)?;
    thread::sleep(Duration::from_millis(50));
    let start = Instant::now();
    drop(backend);
    assert!(start.elapsed() < Duration::from_secs(1));
    Ok(())
}

#[test]
fn invalid_input_reports_error_in_both_open_variants() {
    let absent = Path::new(env!("CARGO_MANIFEST_DIR")).join("video2-no-such-file.mp4");
    assert!(VideoBackend::open_sync(&absent).is_err());
    assert!(block_on(VideoBackend::open(&absent)).is_err());
    assert!(VideoBackend::open_sync(Path::new(env!("CARGO_MANIFEST_DIR"))).is_err());
}

#[test]
fn variable_frame_rate_seek_selects_the_covering_frame() -> Result<()> {
    let fixture = Fixture::generate("0", "setpts=N*N/(25*TB)", false)?;
    let mut backend = VideoBackend::open_sync(&fixture.0)?;
    for (target, expected) in [(500, 360), (1300, 1000), (100, 40), (0, 0)] {
        backend.seek_sync(Duration::from_millis(target))?;
        assert_eq!(
            backend.get_current_frame()?.timestamp,
            Duration::from_millis(expected)
        );
        assert_eq!(backend.position(), Duration::from_millis(target));
    }
    Ok(())
}

#[test]
fn playback_after_seek_keeps_the_lookahead_frame() -> Result<()> {
    let fixture = Fixture::new("0")?;
    let mut backend = VideoBackend::open_sync(&fixture.0)?;
    backend.seek_sync(Duration::from_millis(200))?;
    let deadline = Instant::now() + Duration::from_secs(2);
    backend.set_paused(false)?;
    let mut times = vec![Duration::from_millis(200)];
    while !backend.ended() {
        let time = backend.get_current_frame()?.timestamp;
        if times.last() != Some(&time) {
            times.push(time);
        }
        assert!(Instant::now() < deadline, "playback did not finish");
        thread::sleep(Duration::from_millis(1));
    }
    let mut expected = Vec::new();
    for millis in [200, 240, 280, 320, 360] {
        expected.push(Duration::from_millis(millis));
    }
    assert_eq!(times, expected);
    Ok(())
}

#[test]
fn terminal_errors_are_reported_without_an_update_call() -> Result<()> {
    let fixture = Fixture::new("0")?;
    let mut backend = VideoBackend::open_sync(&fixture.0)?;
    lock(&backend.shared).fail(&anyhow::anyhow!(
        "Injected decoder failure at {}:{}",
        file!(),
        line!()
    ));
    assert!(
        backend
            .get_current_frame()
            .err()
            .context(format!("Expected frame error at {}:{}", file!(), line!()))?
            .to_string()
            .contains("Injected decoder failure")
    );
    assert!(backend.set_paused(false).is_err());
    assert!(backend.seek_sync(Duration::ZERO).is_err());
    Ok(())
}

#[test]
#[ignore = "requires a working default audio output device; output stays muted"]
fn audio_device_playback_seeks_and_finishes_without_frame_reads() -> Result<()> {
    let fixture = Fixture::generate("0", "null", true)?;
    let mut backend = block_on(VideoBackend::open(&fixture.0))?;
    backend.set_muted(true)?;
    // Give paused decoding time to fill its bounded queues before seeking.
    thread::sleep(Duration::from_millis(100));
    block_on(backend.seek(Duration::from_millis(1200)))?;
    assert_eq!(
        backend.get_current_frame()?.timestamp,
        Duration::from_millis(1200)
    );
    block_on(backend.seek(Duration::from_millis(200)))?;
    backend.set_paused(false)?;
    thread::sleep(Duration::from_millis(100));
    block_on(backend.seek(Duration::from_millis(2400)))?;
    assert!(!backend.paused());
    thread::sleep(Duration::from_secs(1));
    backend.get_current_frame()?;
    assert!(backend.ended());
    backend.seek_sync(Duration::ZERO)?;
    assert!(backend.paused());
    drop(backend);
    let tail = Fixture::generate("0", "trim=duration=0.4", true)?;
    let mut backend = block_on(VideoBackend::open(&tail.0))?;
    backend.set_muted(true)?;
    backend.set_paused(false)?;
    thread::sleep(Duration::from_millis(600));
    assert_eq!(
        backend.get_current_frame()?.timestamp,
        Duration::from_millis(360)
    );
    backend.set_paused(true)?;
    block_on(backend.seek(Duration::from_secs(2)))?;
    assert_eq!(
        backend.get_current_frame()?.timestamp,
        Duration::from_millis(360)
    );
    assert_eq!(backend.position(), Duration::from_secs(2));
    backend.set_paused(false)?;
    thread::sleep(Duration::from_millis(1300));
    backend.get_current_frame()?;
    assert!(backend.ended());
    Ok(())
}

struct Fixture(PathBuf);

impl Fixture {
    fn new(offset: &str) -> Result<Self> {
        Self::generate(offset, "null", false)
    }

    fn generate(offset: &str, filter: &str, audio: bool) -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "opencut-video2-{}-{}.mp4",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let fixture = Self(path);
        let executable =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/ffmpeg-8.1.2/bin/ffmpeg");
        let mut command = Command::new(executable);
        command.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=red:size=66x34:rate=25:duration=3",
        ]);
        if audio {
            command.args([
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=44100",
                "-c:a",
                "aac",
                "-t",
                "3",
            ]);
        } else {
            command.args(["-an", "-frames:v", "10"]);
        }
        let output = command
            .args([
                "-c:v",
                "mpeg4",
                "-bf",
                "2",
                "-g",
                "250",
                "-vf",
                filter,
                "-fps_mode",
                "vfr",
                "-output_ts_offset",
                offset,
                "-y",
            ])
            .arg(&fixture.0)
            .output()
            .context(format!(
                "Generating video fixture at {}:{}",
                file!(),
                line!()
            ))?;
        if !output.status.success() {
            bail!(
                "Fixture generation failed: {} at {}:{}",
                String::from_utf8_lossy(&output.stderr),
                file!(),
                line!()
            );
        }
        Ok(fixture)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct ThreadWake(thread::Thread);

impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

#[track_caller]
fn block_on<T>(future: impl Future<Output = T>) -> T {
    let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "async backend operation timed out"
        );
        thread::park_timeout(Duration::from_millis(20));
    }
}
