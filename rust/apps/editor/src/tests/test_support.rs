use ulid::Ulid;

pub(crate) trait TimelineTestExt: Sized {
    fn with_test_tracks() -> Self;
}

pub(crate) fn ulid(value: u64) -> Ulid {
    Ulid::from(u128::from(value))
}
