use super::*;

#[test]
fn parses_integer_bitrate() {
    assert_eq!(parse_bitrate("16").unwrap(), 16_000_000);
}

#[test]
fn rejects_invalid_bitrate() {
    assert!(parse_bitrate("8 Mb/s").is_err());
    assert!(parse_bitrate("fast").is_err());
}

#[test]
fn offers_hardware_and_software_encoders_with_distinct_labels() {
    assert_eq!(EXPORT_ENCODER_PRESETS.len(), 2);
    assert_ne!(
        ExportEncoder::Hardware.label(),
        ExportEncoder::Software.label()
    );
    assert_ne!(
        ExportEncoder::Hardware.implementation(),
        ExportEncoder::Software.implementation()
    );
}

#[test]
fn formats_export_duration() {
    assert_eq!(format_export_duration(Duration::from_secs(1)), "1 second");
    assert_eq!(
        format_export_duration(Duration::from_secs(42)),
        "42 seconds"
    );
    assert_eq!(
        format_export_duration(Duration::from_secs(128)),
        "2 min 8 seconds"
    );
}
