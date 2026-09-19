use super::*;
use crate::video2::tests::block_on;
use std::{fs, io::Write, path::PathBuf};

#[test]
fn seek_submits_immediately_clamps_and_preserves_pause_changes() -> Result<()> {
    let shared = Arc::new(Mutex::new(State::new()));
    lock(&shared).duration = Duration::from_secs(3);
    let (commands, requests) = mpsc::channel();
    let mut backend = AudioBackend {
        shared,
        commands,
        worker: None,
    };
    backend.set_paused(false)?;
    let completion = backend.seek(Duration::from_secs(20));
    let request = requests.try_recv()?;
    assert_eq!(request.position, Duration::from_secs(3));
    assert_eq!(request.generation, 1);
    assert!(!backend.paused());
    backend.set_paused(true)?;
    assert!(backend.paused());
    request.reply.try_send(Ok(()))?;
    block_on(completion)?;
    let second = backend.seek(Duration::from_secs(1));
    let request = requests.try_recv()?;
    assert_eq!(request.generation, 2);
    assert!(backend.paused());
    request.reply.try_send(Ok(()))?;
    block_on(second)?;
    Ok(())
}

#[test]
fn audio_errors_propagate_and_volume_is_validated() -> Result<()> {
    let (commands, requests) = mpsc::channel();
    let mut backend = AudioBackend {
        shared: Arc::new(Mutex::new(State::new())),
        commands,
        worker: None,
    };
    backend.set_volume(2.0)?;
    assert_eq!(backend.volume(), 1.0);
    backend.set_volume(-1.0)?;
    assert_eq!(backend.volume(), 0.0);
    assert!(backend.set_volume(f64::NAN).is_err());
    assert!(backend.set_volume(f64::INFINITY).is_err());
    let completion = backend.seek(Duration::ZERO);
    let request = requests.try_recv()?;
    lock(&backend.shared).fail(&anyhow!("device disconnected"));
    drop(request);
    assert!(format!("{:?}", block_on(completion).unwrap_err()).contains("device disconnected"));
    assert!(backend.check().is_err());
    assert!(backend.set_paused(false).is_err());
    assert!(backend.set_volume(0.4).is_err());
    Ok(())
}

#[test]
fn opening_invalid_audio_returns_an_error() {
    assert!(block_on(AudioBackend::open(Path::new(env!("CARGO_MANIFEST_DIR")))).is_err());
    assert!(
        block_on(AudioBackend::open(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
        ))
        .is_err()
    );
}

#[test]
#[ignore = "requires a working default audio output device; fixture contains silence"]
fn audio_only_playback_seeks_finishes_replays_and_shuts_down() -> Result<()> {
    let fixture = AudioFixture::new()?;
    let mut backend = block_on(AudioBackend::open(&fixture.0))?;
    assert!(backend.paused());
    assert_eq!(backend.duration(), Duration::from_secs(3));
    assert_eq!(backend.position(), Duration::ZERO);
    backend.set_volume(0.0)?;
    thread::sleep(Duration::from_millis(100));
    assert!(!backend.ended());
    block_on(backend.seek(Duration::from_millis(1200)))?;
    assert_eq!(backend.position(), Duration::from_millis(1200));
    block_on(backend.seek(Duration::from_millis(200)))?;
    assert!(backend.paused());
    backend.set_paused(false)?;
    thread::sleep(Duration::from_millis(80));
    assert!(backend.position() > Duration::from_millis(200));
    block_on(backend.seek(Duration::from_millis(2800)))?;
    assert!(!backend.paused());
    let deadline = Instant::now() + Duration::from_secs(3);
    while !backend.ended() {
        backend.check()?;
        assert!(Instant::now() < deadline, "audio did not reach EOF");
        thread::sleep(Duration::from_millis(10));
    }
    assert!(backend.paused());
    assert!(backend.position().abs_diff(Duration::from_secs(3)) < Duration::from_millis(50));
    block_on(backend.seek(Duration::ZERO))?;
    assert!(!backend.ended());
    backend.set_paused(false)?;
    thread::sleep(Duration::from_millis(80));
    assert!(backend.position() > Duration::ZERO);
    backend.set_paused(true)?;
    let position = backend.position();
    thread::sleep(Duration::from_millis(50));
    assert_eq!(backend.position(), position);
    let started = Instant::now();
    drop(backend);
    assert!(started.elapsed() < Duration::from_secs(2));
    Ok(())
}

struct AudioFixture(PathBuf);

impl AudioFixture {
    fn new() -> Result<Self> {
        let path =
            std::env::temp_dir().join(format!("opencut-audio-only-{}.wav", std::process::id()));
        let fixture = Self(path);
        let mut file = fs::File::create(&fixture.0)?;
        let rate = 48_000_u32;
        let bytes = rate * 3 * 2;
        file.write_all(b"RIFF")?;
        file.write_all(&(36 + bytes).to_le_bytes())?;
        file.write_all(b"WAVEfmt ")?;
        file.write_all(&16_u32.to_le_bytes())?;
        file.write_all(&1_u16.to_le_bytes())?;
        file.write_all(&1_u16.to_le_bytes())?;
        file.write_all(&rate.to_le_bytes())?;
        file.write_all(&(rate * 2).to_le_bytes())?;
        file.write_all(&2_u16.to_le_bytes())?;
        file.write_all(&16_u16.to_le_bytes())?;
        file.write_all(b"data")?;
        file.write_all(&bytes.to_le_bytes())?;
        file.write_all(&vec![0; bytes as usize])?;
        Ok(fixture)
    }
}

impl Drop for AudioFixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
