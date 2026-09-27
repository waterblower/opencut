use ulid::Ulid;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TrackKind {
    #[default]
    Video,
    Audio,
    Text,
}

#[derive(Clone, Debug)]
pub struct Track {
    pub id: Ulid,
    pub name: String,
    pub kind: TrackKind,
    pub locked: bool,
    pub muted: bool,
    pub visible: bool,
}
