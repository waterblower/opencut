//! Run from `rust` with:
//! cargo test --config .cargo/macos.toml -p media-backend --test random_seek -- --ignored --nocapture

use anyhow::{Context, Result, ensure};
use media_backend::{VideoBackend, VideoDecoder};
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[test]
#[ignore = "local video fixtures and hardware-dependent performance benchmark"]
fn benchmark_random_seek() -> Result<()> {
    const SEEK_COUNT: usize = 32;
    const SEED: u64 = 0x6f70_656e_6375_7421;
    let fixture_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(".testdata");

    // fake-keyframes.mp4：容器同步样本表把非 IDR 的 I 帧标成关键帧。
    for name in [
        "short.mp4",
        "4K.MOV",
        "super-long.mp4",
        "fake-keyframes.mp4",
    ] {
        let path = fixture_directory.join(name);
        let metadata = VideoBackend::probe(&path)
            .with_context(|| format!("probing benchmark fixture {}", path.display()))?;
        ensure!(
            !metadata.duration.is_zero(),
            "empty benchmark fixture {name}"
        );
        let mut decoder = VideoDecoder::open(
            &path,
            metadata.video.stream_index,
            metadata.origin_microseconds,
        )?;
        let mut random_state = SEED;
        let mut timings = Vec::with_capacity(SEEK_COUNT);
        let mut worst_target = Duration::ZERO;
        let mut worst_elapsed = Duration::ZERO;
        println!(
            "\n{name}: {}x{}, duration={:?}, seeks={SEEK_COUNT}, seed={SEED:#x}",
            metadata.video.width, metadata.video.height, metadata.duration
        );

        for index in 0..SEEK_COUNT {
            // 固定序列使每次运行选择相同位置；各视频使用相同的相对位置。
            random_state = random_state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1);
            let fraction = (random_state >> 32) as f64 / (u32::MAX as f64 + 1.0);
            let target = metadata.duration.mul_f64(fraction);
            let started = Instant::now();
            decoder
                .seek(target)
                .with_context(|| format!("{name}: seek {} to {target:?}", index + 1))?;
            let frame = decoder
                .next_frame()?
                .with_context(|| format!("{name}: no frame after seeking to {target:?}"))?;
            let elapsed = started.elapsed();
            timings.push(elapsed);
            if elapsed > worst_elapsed {
                worst_elapsed = elapsed;
                worst_target = target;
            }
            println!(
                "  {:02}: target={target:?}, landed={:?}, elapsed={elapsed:?}",
                index + 1,
                frame.timestamp
            );
        }

        let total: Duration = timings.iter().copied().sum();
        timings.sort_unstable();
        // 分位数使用 nearest-rank；不以机器相关的耗时阈值判定测试成败。
        println!(
            "{name}: mean={:?}, p50={:?}, p95={:?}, max={worst_elapsed:?} at {worst_target:?}, total={total:?}",
            total / SEEK_COUNT as u32,
            timings[SEEK_COUNT.div_ceil(2) - 1],
            timings[(SEEK_COUNT * 95).div_ceil(100) - 1]
        );
    }
    Ok(())
}
