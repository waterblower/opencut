use super::*;

#[test]
fn generates_waveform() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/tests/long video.mp4");
    let waveform = generate_waveform(&source).unwrap();

    assert!(waveform.sample_rate > 0);
    assert!(waveform.total_samples > 0);
    assert!(waveform.levels.len() > 1);
    assert_eq!(waveform.levels[0].samples_per_peak, 64);
    assert_eq!(waveform.columns(0.0, 1.0, 320).len(), 320);
}

#[test]
fn groups_samples_across_decoded_buffers() {
    let mut builder = WaveformBuilder::new(4);
    builder.push_samples(&[0.5, -0.5]);
    builder.push_samples(&[2.0, f32::NAN, 0.25]);

    assert_eq!(builder.total_samples, 5);
    assert_eq!(
        builder.finish(),
        vec![
            WaveformPeak {
                min: -0.5,
                max: 1.0,
            },
            WaveformPeak {
                min: 0.25,
                max: 0.25,
            },
        ]
    );
}
