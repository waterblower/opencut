use super::*;
use ffmpeg_next as ffmpeg;

#[test]
#[ignore = "benchmarks the large local fixture and requires a default audio device"]
fn long_video_random_forward_backward_seeks() -> Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/tests/long video.mp4");
    let mut backend = VideoBackend::open_sync(&path)?;
    backend.set_muted(true)?;
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
        let start = Instant::now();
        backend.seek_sync(*target)?;
        elapsed.push(start.elapsed());
        let frame = backend.get_current_frame()?;
        assert_eq!(frame.timestamp, timestamp);
        assert_eq!(pixels_checksum(&frame.image), checksum);
        assert_eq!(backend.position(), *target);
        assert!(backend.paused());
    }
    report("random full-file", &mut elapsed);

    // Warm a neighborhood through a real seek, then sample distinct fractional
    // timestamps in both directions. This is not the exact-target no-op path.
    let anchor = Duration::from_secs_f64(502.8);
    backend.seek_sync(anchor)?;
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
        let start = Instant::now();
        backend.seek_sync(*target)?;
        elapsed.push(start.elapsed());
        let frame = backend.get_current_frame()?;
        assert_eq!(frame.timestamp, timestamp);
        assert_eq!(pixels_checksum(&frame.image), checksum);
        assert_eq!(backend.position(), *target);
    }
    report("random cached neighborhood", &mut elapsed);
    // Timing is reported, not asserted: OS scheduling and concurrent workloads
    // make wall-clock thresholds unsuitable as deterministic correctness tests.
    Ok(())
}

fn report(label: &str, times: &mut [Duration]) {
    times.sort_unstable();
    let budget = Duration::from_micros(8300);
    eprintln!(
        "{label}: n={} p50={:.2}ms p95={:.2}ms max={:.2}ms <=8.3ms={}/{}",
        times.len(),
        times[times.len() / 2].as_secs_f64() * 1000.0,
        times[(times.len() * 95).div_ceil(100) - 1].as_secs_f64() * 1000.0,
        times[times.len() - 1].as_secs_f64() * 1000.0,
        times.iter().filter(|time| **time <= budget).count(),
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
    assert_eq!(frame.format(), ffmpeg::format::Pixel::YUV420P);
    let mut hash = 14695981039346656037_u64;
    for plane in 0..frame.planes() {
        for row in 0..frame.plane_height(plane) as usize {
            let start = row * frame.stride(plane);
            for byte in &frame.data(plane)[start..start + frame.plane_width(plane) as usize] {
                hash = (hash ^ u64::from(*byte)).wrapping_mul(1099511628211);
            }
        }
    }
    hash
}
