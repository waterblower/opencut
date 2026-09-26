use super::*;
use ffmpeg_next as ffmpeg;

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires native VideoToolbox hardware; generates short AVC/HEVC fixtures"]
fn hardware_reordering_vfr_seek_playback_and_frame_lifetime() -> Result<()> {
    for (codec, filter, origin) in [
        ("libx264", "null", "0"),
        ("libx265", "select='not(eq(mod(n,5),2))'", "2"),
    ] {
        let fixture = HardwareFixture(std::env::temp_dir().join(format!(
            "opencut-videotoolbox-{}-{codec}.mp4",
            std::process::id()
        )));
        let executable =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/ffmpeg-8.1.2/bin/ffmpeg");
        let mut command = std::process::Command::new(executable);
        command.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=128x96:rate=25:duration=2",
        ]);
        if codec == "libx265" {
            command.args([
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=44100",
                "-t",
                "2",
                "-c:a",
                "aac",
            ]);
        } else {
            command.arg("-an");
        }
        command.args([
            "-c:v",
            codec,
            "-g",
            "25",
            "-bf",
            "3",
            "-pix_fmt",
            "yuv420p",
            "-vf",
            filter,
            "-fps_mode",
            "vfr",
            "-output_ts_offset",
            origin,
        ]);
        if codec == "libx265" {
            command.args(["-x265-params", "log-level=error"]);
        }
        let output = command.arg("-y").arg(&fixture.0).output().context(format!(
            "Generating hardware fixture at {}:{}",
            file!(),
            line!()
        ))?;
        if !output.status.success() {
            bail!(
                "Hardware fixture failed: {} at {}:{}",
                String::from_utf8_lossy(&output.stderr),
                file!(),
                line!()
            );
        }
        let mut backend = VideoBackend::open_sync(&fixture.0)?;
        backend.set_muted(true)?;
        let retained = backend.get_current_frame()?;
        assert_eq!(retained.image.format(), ffmpeg::format::Pixel::NV12);
        let original = pixels_checksum(&retained.image);
        for millis in [1125, 1850, 215, 777, 40, 1080, 720, 721, 1959, 0, 399] {
            let target = Duration::from_millis(millis);
            let expected = reference(&fixture.0, target)?;
            backend.seek_sync(target)?;
            let image = backend.get_current_frame()?;
            assert_eq!(
                (image.timestamp, pixels_checksum(&image.image)),
                expected,
                "{codec} at {target:?}"
            );
            assert!(backend.paused());
        }
        // Complete playback without application frame reads, including the last
        // partial hardware batch and B-frame reordering drain.
        backend.seek_sync(Duration::from_millis(1500))?;
        backend.set_paused(false)?;
        thread::sleep(Duration::from_millis(800));
        assert!(backend.ended());
        let last = reference(&fixture.0, Duration::from_secs(3))?;
        let image = backend.get_current_frame()?;
        assert_eq!((image.timestamp, pixels_checksum(&image.image)), last);
        backend.seek_sync(Duration::ZERO)?;
        assert!(backend.paused());
        // A new seek supersedes work while the paused presentation queue is full.
        let abandoned = backend.seek(Duration::from_millis(1250));
        backend.seek_sync(Duration::from_millis(320))?;
        drop(abandoned);
        drop(backend);
        // Mapping lifetime is tied to frame references, not the native session.
        assert_eq!(pixels_checksum(&retained.image), original);
        assert_eq!(pixels_checksum(&retained.image.clone()), original);
        if codec == "libx264" {
            // Length-prefixed AVC alone is insufficient: containers without
            // MOV's timestamp guarantees must retain software decoding.
            let remuxed = HardwareFixture(fixture.0.with_extension("mkv"));
            let executable =
                Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/ffmpeg-8.1.2/bin/ffmpeg");
            let output = std::process::Command::new(executable)
                .args(["-hide_banner", "-loglevel", "error", "-i"])
                .arg(&fixture.0)
                .args(["-map", "0:v:0", "-c", "copy", "-y"])
                .arg(&remuxed.0)
                .output()
                .context(format!(
                    "Remuxing fallback fixture at {}:{}",
                    file!(),
                    line!()
                ))?;
            if !output.status.success() {
                bail!(
                    "Fallback fixture failed: {} at {}:{}",
                    String::from_utf8_lossy(&output.stderr),
                    file!(),
                    line!()
                );
            }
            let mut software = VideoBackend::open_sync(&remuxed.0)?;
            assert_eq!(
                software.get_current_frame()?.image.format(),
                ffmpeg::format::Pixel::YUV420P
            );
            let target = Duration::from_millis(777);
            software.seek_sync(target)?;
            let image = software.get_current_frame()?;
            assert_eq!(
                (image.timestamp, pixels_checksum(&image.image)),
                reference(&remuxed.0, target)?
            );
        }
    }
    Ok(())
}

#[test]
#[ignore = "long-running raw-seek workload for CPU profiling; needs an audio device"]
fn raw_seek_profile() -> Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/tests/long video.mp4");
    let targets = [
        Duration::from_secs_f64(116.690645),
        Duration::from_secs_f64(135.963488),
    ];
    let expected = [reference(&path, targets[0])?, reference(&path, targets[1])?];
    eprintln!("seek profiling PID={}", std::process::id());
    let mut elapsed = Vec::new();
    // A fresh backend per pair prevents sparse preroll's small output cache
    // from turning this workload into repeated cache hits. Open is untimed.
    for pair in 0..50 {
        let mut backend = VideoBackend::open_sync(&path)?;
        backend.set_muted(true)?;
        #[cfg(target_os = "macos")]
        assert_eq!(
            backend.get_current_frame()?.image.format(),
            ffmpeg::format::Pixel::NV12
        );
        for offset in 0..2 {
            let index = (pair + offset) % 2;
            let target = targets[index];
            let from = backend.position();
            let started = Instant::now();
            backend.seek_sync(target)?;
            elapsed.push(SeekTiming {
                elapsed: started.elapsed(),
                from,
                target,
            });
            let frame = backend.get_current_frame()?;
            assert_eq!(
                (frame.timestamp, pixels_checksum(&frame.image)),
                expected[index]
            );
        }
    }
    report(
        "raw cold long-GOP seeks (fresh backend per pair)",
        &mut elapsed,
    );
    Ok(())
}

#[test]
#[ignore = "benchmarks the large local fixture and requires a default audio device"]
fn long_video_random_forward_backward_seeks() -> Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/tests/long video.mp4");
    let mut backend = VideoBackend::open_sync(&path)?;
    backend.set_muted(true)?;
    #[cfg(target_os = "macos")]
    assert_eq!(
        backend.get_current_frame()?.image.format(),
        ffmpeg::format::Pixel::NV12,
        "hardware benchmark must not silently use software decoding"
    );
    let mut random = 0x1735_abcd_u64;
    let mut targets = Vec::new();
    // Alternate random positions in the upper/lower halves: every operation
    // changes direction and traverses a different part of the actual file.
    for index in 0..24 {
        random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
        let fraction = ((random >> 32) as f64 / u32::MAX as f64) * 0.45;
        targets.push(
            backend
                .duration()
                .mul_f64(fraction + if index % 2 == 0 { 0.5 } else { 0.01 }),
        );
    }
    // Build independent decoded references outside the timed region.
    let mut expected = Vec::new();
    for target in &targets {
        expected.push(reference(&path, *target)?);
    }
    let mut elapsed = Vec::new();
    for (target, (timestamp, checksum)) in targets.iter().zip(expected) {
        let from = backend.position();
        let start = Instant::now();
        backend.seek_sync(*target)?;
        elapsed.push(SeekTiming {
            elapsed: start.elapsed(),
            from,
            target: *target,
        });
        let frame = backend.get_current_frame()?;
        assert_eq!(frame.timestamp, timestamp);
        assert_eq!(pixels_checksum(&frame.image), checksum);
        assert_eq!(backend.position(), *target);
        assert!(backend.paused());
    }
    report("random full-file", &mut elapsed);
    let mut overall = elapsed.clone();

    // Warm a neighborhood through a real seek, then sample distinct fractional
    // timestamps in both directions. This is not the exact-target no-op path.
    let anchor = Duration::from_secs_f64(502.8);
    let from = backend.position();
    let start = Instant::now();
    backend.seek_sync(anchor)?;
    overall.push(SeekTiming {
        elapsed: start.elapsed(),
        from,
        target: anchor,
    });
    targets.clear();
    for _ in 0..48 {
        random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
        let offset = 0.02 + ((random >> 32) as f64 / u32::MAX as f64) * 0.3;
        targets.push(anchor - Duration::from_secs_f64(offset));
    }
    expected = Vec::new();
    for target in &targets {
        expected.push(reference(&path, *target)?);
    }
    elapsed.clear();
    for (target, (timestamp, checksum)) in targets.iter().zip(expected) {
        let from = backend.position();
        let start = Instant::now();
        backend.seek_sync(*target)?;
        elapsed.push(SeekTiming {
            elapsed: start.elapsed(),
            from,
            target: *target,
        });
        let frame = backend.get_current_frame()?;
        assert_eq!(frame.timestamp, timestamp);
        assert_eq!(pixels_checksum(&frame.image), checksum);
        assert_eq!(backend.position(), *target);
    }
    report("random nearby seeks (cache may miss)", &mut elapsed);
    overall.extend(elapsed);

    // Cross cache boundaries while scrubbing through several GOPs. Forward
    // misses can continue the live decoder; backward misses must really seek.
    targets = (0..32)
        .map(|index| Duration::from_secs_f64(5.125 + f64::from(index) * 0.613))
        .collect();
    targets.extend(targets.clone().into_iter().rev().skip(1));
    expected = Vec::new();
    for target in &targets {
        expected.push(reference(&path, *target)?);
    }
    elapsed = Vec::new();
    for (target, (timestamp, checksum)) in targets.iter().zip(expected) {
        let from = backend.position();
        let start = Instant::now();
        backend.seek_sync(*target)?;
        elapsed.push(SeekTiming {
            elapsed: start.elapsed(),
            from,
            target: *target,
        });
        let frame = backend.get_current_frame()?;
        assert_eq!(frame.timestamp, timestamp, "sweep target={target:?}");
        assert_eq!(pixels_checksum(&frame.image), checksum);
        assert_eq!(backend.position(), *target);
        assert!(backend.paused());
    }
    report("forward/backward scrub", &mut elapsed);
    let mut forward: Vec<_> = elapsed
        .iter()
        .copied()
        .filter(|time| time.target > time.from)
        .collect();
    let mut backward: Vec<_> = elapsed
        .iter()
        .copied()
        .filter(|time| time.target < time.from)
        .collect();
    report("forward scrub", &mut forward);
    report("backward scrub (including initial jump)", &mut backward);
    overall.extend(elapsed);
    report("overall (including cache warmup)", &mut overall);
    // Timing is reported, not asserted: OS scheduling and concurrent workloads
    // make wall-clock thresholds unsuitable as deterministic correctness tests.
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SeekTiming {
    // Sort primarily by elapsed time, keeping the corresponding seek attached.
    elapsed: Duration,
    from: Duration,
    target: Duration,
}

#[cfg(target_os = "macos")]
struct HardwareFixture(std::path::PathBuf);

#[cfg(target_os = "macos")]
impl Drop for HardwareFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn report(label: &str, times: &mut [SeekTiming]) {
    times.sort_unstable();
    let worst = times.last().expect("benchmark contains seeks");
    let budget = Duration::from_micros(8300);
    eprintln!(
        "{label}: n={} worst={:.2}ms from={:.6}s target={:.6}s direction={} p50={:.2}ms p95={:.2}ms <=8.3ms={}/{}",
        times.len(),
        worst.elapsed.as_secs_f64() * 1000.0,
        worst.from.as_secs_f64(),
        worst.target.as_secs_f64(),
        if worst.target < worst.from {
            "backward"
        } else if worst.target > worst.from {
            "forward"
        } else {
            "unchanged"
        },
        times[times.len() / 2].elapsed.as_secs_f64() * 1000.0,
        times[(times.len() * 95).div_ceil(100) - 1]
            .elapsed
            .as_secs_f64()
            * 1000.0,
        times.iter().filter(|time| time.elapsed <= budget).count(),
        times.len(),
    );
}

fn reference(path: &Path, target: Duration) -> Result<(Duration, u64)> {
    let mut input = ffmpeg::format::input(path).context(format!(
        "Opening reference at {}:{}",
        file!(),
        line!()
    ))?;
    let stream = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .context(format!("Reference video track at {}:{}", file!(), line!()))?;
    let index = stream.index();
    let base = f64::from(stream.time_base());
    let origin = if stream.start_time() == ffmpeg::ffi::AV_NOPTS_VALUE {
        0.0
    } else {
        stream.start_time() as f64 * base
    };
    let mut context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
        .context(format!("Reference parameters at {}:{}", file!(), line!()))?;
    context.set_threading(ffmpeg::codec::threading::Config::kind(
        ffmpeg::codec::threading::Type::Frame,
    ));
    let mut video = context.decoder().video().context(format!(
        "Reference decoder at {}:{}",
        file!(),
        line!()
    ))?;
    let timestamp = ((target.as_secs_f64() + origin) * ffmpeg::ffi::AV_TIME_BASE as f64) as i64;
    input.seek(timestamp, ..timestamp).context(format!(
        "Seeking reference at {}:{}",
        file!(),
        line!()
    ))?;
    let mut selected = None;
    let mut draining = false;
    loop {
        let mut image = ffmpeg::frame::Video::empty();
        match video.receive_frame(&mut image) {
            Ok(()) => {
                let timestamp = seconds(
                    image.timestamp().context(format!(
                        "Reference PTS at {}:{}",
                        file!(),
                        line!()
                    ))? as f64
                        * base
                        - origin,
                );
                if timestamp > target {
                    let (timestamp, frame) = selected.context(format!(
                        "Missing reference frame at {}:{}",
                        file!(),
                        line!()
                    ))?;
                    return Ok((timestamp, pixels_checksum(&frame)));
                }
                selected = Some((timestamp, image));
                continue;
            }
            Err(ffmpeg::Error::Eof) => {
                let (timestamp, frame) =
                    selected.context(format!("Empty reference at {}:{}", file!(), line!()))?;
                return Ok((timestamp, pixels_checksum(&frame)));
            }
            Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => {}
            Err(error) => bail!(
                "Reading reference frame at {}:{}: {error}",
                file!(),
                line!()
            ),
        }
        assert!(!draining);
        let mut packet = ffmpeg::Packet::empty();
        match packet.read(&mut input) {
            Ok(()) if packet.stream() == index => video.send_packet(&packet).context(format!(
                "Reference packet at {}:{}",
                file!(),
                line!()
            ))?,
            Ok(()) => {}
            Err(ffmpeg::Error::Eof) => {
                video.send_eof().context(format!(
                    "Draining reference at {}:{}",
                    file!(),
                    line!()
                ))?;
                draining = true;
            }
            Err(error) => bail!(
                "Reading reference packet at {}:{}: {error}",
                file!(),
                line!()
            ),
        }
    }
}

fn pixels_checksum(frame: &ffmpeg::frame::Video) -> u64 {
    assert!(matches!(
        frame.format(),
        ffmpeg::format::Pixel::YUV420P | ffmpeg::format::Pixel::NV12
    ));
    let mut hash = 14695981039346656037_u64;
    // Hash logical Y, U, V values, independent of planar/interleaved storage.
    for component in 0..3 {
        let chroma = component != 0;
        let interleaved = chroma && frame.format() == ffmpeg::format::Pixel::NV12;
        let plane = if interleaved { 1 } else { component };
        let width = if chroma {
            frame.width().div_ceil(2)
        } else {
            frame.width()
        };
        let height = if chroma {
            frame.height().div_ceil(2)
        } else {
            frame.height()
        };
        let data = frame.data(plane);
        for row in 0..height as usize {
            let start = row * frame.stride(plane);
            for column in 0..width as usize {
                let offset = if interleaved {
                    column * 2 + component - 1
                } else {
                    column
                };
                hash = (hash ^ u64::from(data[start + offset])).wrapping_mul(1099511628211);
            }
        }
    }
    hash
}
