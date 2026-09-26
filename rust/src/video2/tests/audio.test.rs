use super::*;

#[test]
fn pause_retains_audio_and_resume_consumes_each_sample_once_with_gain() -> Result<()> {
    let (sender, blocks) = mpsc::sync_channel(4);
    sender
        .send(Block {
            generation: 0,
            time: 0.0,
            samples: vec![1.0, -1.0, 0.5, -0.5, 0.25, -0.25, 0.125, -0.125],
        })
        .context(format!("Queuing test audio at {}:{}", file!(), line!()))?;
    let mut output = Output {
        blocks,
        pending: None,
        cursor: 0,
        channels: 2,
        rate: 4.0,
    };
    let now = Instant::now();
    let mut snapshot = Snapshot {
        clock: Clock::Paused(Duration::ZERO),
        generation: 0,
        gain: 0.5,
    };
    let mut buffer = [9.0; 4];
    output.fill(&mut buffer, snapshot, now);
    assert_eq!(buffer, [0.0; 4]);
    snapshot.clock = Clock::Playing {
        position: Duration::ZERO,
        since: now,
    };
    output.fill(&mut buffer, snapshot, now);
    assert_eq!(buffer, [0.5, -0.5, 0.25, -0.25]);
    output.fill(&mut buffer, snapshot, now + Duration::from_millis(500));
    assert_eq!(buffer, [0.125, -0.125, 0.0625, -0.0625]);
    output.fill(&mut buffer, snapshot, now + Duration::from_secs(1));
    assert_eq!(buffer, [0.0; 4]);
    Ok(())
}

#[test]
fn seek_discards_pending_and_queued_audio_from_previous_generation() -> Result<()> {
    let (sender, blocks) = mpsc::sync_channel(4);
    sender
        .send(Block {
            generation: 1,
            time: 0.0,
            samples: vec![1.0; 8],
        })
        .context(format!("Queuing stale audio at {}:{}", file!(), line!()))?;
    sender
        .send(Block {
            generation: 2,
            time: 5.0,
            samples: vec![0.25; 4],
        })
        .context(format!("Queuing seek audio at {}:{}", file!(), line!()))?;
    let mut output = Output {
        blocks,
        pending: Some(Block {
            generation: 1,
            time: 0.0,
            samples: vec![1.0; 4],
        }),
        cursor: 2,
        channels: 2,
        rate: 4.0,
    };
    let now = Instant::now();
    let mut snapshot = Snapshot {
        clock: Clock::Seeking {
            position: Duration::ZERO,
            resume: true,
        },
        generation: 2,
        gain: 1.0,
    };
    let mut buffer = [9.0; 4];
    output.fill(&mut buffer, snapshot, now);
    assert_eq!(buffer, [0.0; 4]);
    snapshot.clock = Clock::Playing {
        position: Duration::from_secs(5),
        since: now,
    };
    output.fill(&mut buffer, snapshot, now);
    assert_eq!(buffer, [0.25; 4]);
    Ok(())
}

#[test]
fn mute_consumes_audio_and_future_audio_waits_for_its_timestamp() -> Result<()> {
    let (sender, blocks) = mpsc::sync_channel(4);
    sender
        .send(Block {
            generation: 0,
            time: 0.0,
            samples: vec![1.0; 4],
        })
        .context(format!("Queuing muted audio at {}:{}", file!(), line!()))?;
    sender
        .send(Block {
            generation: 0,
            time: 1.0,
            samples: vec![0.25; 4],
        })
        .context(format!("Queuing future audio at {}:{}", file!(), line!()))?;
    let mut output = Output {
        blocks,
        pending: None,
        cursor: 0,
        channels: 2,
        rate: 4.0,
    };
    let now = Instant::now();
    let mut snapshot = Snapshot {
        clock: Clock::Playing {
            position: Duration::ZERO,
            since: now,
        },
        generation: 0,
        gain: 0.0,
    };
    let mut buffer = [9.0; 4];
    output.fill(&mut buffer, snapshot, now);
    assert_eq!(buffer, [0.0; 4]);
    snapshot.gain = 1.0;
    output.fill(&mut buffer, snapshot, now + Duration::from_millis(500));
    assert_eq!(buffer, [0.0; 4]);
    output.fill(&mut buffer, snapshot, now + Duration::from_secs(1));
    assert_eq!(buffer, [0.25; 4]);
    Ok(())
}
