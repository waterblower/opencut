use ulid::Ulid;

pub trait TimelineTestExt: Sized {
    fn with_test_tracks() -> Self;
}

pub fn ulid(value: u64) -> Ulid {
    Ulid::from(u128::from(value))
}
