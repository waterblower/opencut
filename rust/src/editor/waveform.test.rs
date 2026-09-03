use super::*;

#[test]
fn generates_waveform() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/tests/super long.mp4");
    let waveform = generate_waveform(&source).unwrap();

    assert!(waveform.sample_rate > 0);
    assert!(waveform.total_samples > 0);
    assert!(waveform.levels.len() > 1);
    assert_eq!(waveform.levels[0].samples_per_peak, 64);
    assert_eq!(waveform.columns(0.0, 1.0, 320).len(), 320);
}

#[test]
fn generates_waveform_gstreamer() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/tests/super long.mp4");
    let waveform = generate_waveform_gstreamer(&source).unwrap();

    assert!(waveform.sample_rate > 0);
    assert!(waveform.total_samples > 0);
    assert!(waveform.levels.len() > 1);
    assert_eq!(waveform.levels[0].samples_per_peak, 64);
    assert_eq!(waveform.columns(0.0, 1.0, 320).len(), 320);
}
