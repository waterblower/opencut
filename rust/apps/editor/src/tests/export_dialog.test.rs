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
fn formats_export_duration() {
    for (seconds, expected) in [
        (1, "1 second"),
        (42, "42 seconds"),
        (128, "2 min 8 seconds"),
    ] {
        assert_eq!(
            format_export_duration(Duration::from_secs(seconds)),
            expected
        );
    }
}
