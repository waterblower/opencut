use super::*;

#[test]
fn format_time_unpadded_when_less_than_hour() {
    assert_eq!(format_time(59.2, false), "0:59");
    assert_eq!(format_time(125.0, false), "2:05");
}

#[test]
fn format_time_padded_minutes_in_explorer_style() {
    assert_eq!(format_time(59.2, true), "00:59");
    assert_eq!(format_time(125.0, true), "02:05");
}

#[test]
fn format_time_shows_hours_with_zero_padded_minutes() {
    assert_eq!(format_time(3661.0, false), "1:01:01");
}

#[test]
fn format_time_rounds_and_clamps_negative_input() {
    assert_eq!(format_time(-1.0, false), "0:00");
    assert_eq!(format_time(59.5, true), "01:00");
}
