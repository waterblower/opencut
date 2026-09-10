use crate::{
    cli::{document::Document, error::Result, validate::MediaInfo},
    cli_error, cli_try,
};
use ffmpeg_next as ffmpeg;
use serde::Serialize;
use std::{collections::HashMap, path::Path};

#[derive(Debug, Serialize)]
pub struct Probe {
    pub container: String,
    pub duration: f64,
    pub streams: Vec<Stream>,
    pub keyframe_interval_s: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct Stream {
    pub index: usize,
    pub kind: String,
    pub codec: String,
    pub codec_tag: String,
    pub bitrate: Option<u64>,
    pub width: u32,
    pub height: u32,
    pub fps: Option<[i32; 2]>,
    pub sample_rate: u32,
    pub channels: u16,
    pub rotation: f64,
}

pub fn init() -> Result<()> {
    cli_try!(ffmpeg::init(), "ffmpeg_init", "", 5);
    ffmpeg::log::set_level(ffmpeg::log::Level::Quiet);
    Ok(())
}

pub fn is_image(path: &Path) -> bool {
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" | "ico" | "svg"
    )
}

pub fn probe(path: &Path) -> Result<Probe> {
    if is_image(path) {
        let image = crate::cli::engine::raster::load_image(path)?;
        return Ok(Probe {
            container: "image".into(),
            duration: 0.0,
            keyframe_interval_s: None,
            streams: vec![Stream {
                index: 0,
                kind: "video".into(),
                codec: path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                codec_tag: String::new(),
                bitrate: None,
                width: image.width(),
                height: image.height(),
                fps: None,
                sample_rate: 0,
                channels: 0,
                rotation: 0.0,
            }],
        });
    }
    init()?;
    let mut input = cli_try!(ffmpeg::format::input(path), "unreadable_media", "", 4);
    let mut streams = Vec::new();
    let mut duration = (input.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64).max(0.0);
    let mut video_index = None;
    let mut video_base = 0.0;
    for stream in input.streams() {
        let params = stream.parameters();
        let bitrate = unsafe { (*params.as_ptr()).bit_rate };
        let tag = unsafe { (*params.as_ptr()).codec_tag };
        let tag_bytes = tag.to_le_bytes();
        let codec_tag = if tag_bytes.iter().all(u8::is_ascii_graphic) {
            String::from_utf8_lossy(&tag_bytes).into_owned()
        } else {
            format!("0x{tag:08x}")
        };
        let mut item = Stream {
            index: stream.index(),
            kind: "other".into(),
            codec: params.id().name().into(),
            codec_tag,
            bitrate: if bitrate > 0 {
                Some(bitrate as u64)
            } else {
                None
            },
            width: 0,
            height: 0,
            fps: None,
            sample_rate: 0,
            channels: 0,
            rotation: 0.0,
        };
        if stream.duration() > 0 {
            duration = duration.max(stream.duration() as f64 * f64::from(stream.time_base()));
        }
        let context = cli_try!(
            ffmpeg::codec::context::Context::from_parameters(params.clone()),
            "unreadable_media",
            "",
            4
        );
        match params.medium() {
            ffmpeg::media::Type::Video => {
                let decoder = cli_try!(context.decoder().video(), "unreadable_media", "", 4);
                item.kind = "video".into();
                item.width = decoder.width();
                item.height = decoder.height();
                let rate = stream.avg_frame_rate();
                if rate.denominator() > 0 {
                    item.fps = Some([rate.numerator(), rate.denominator()]);
                }
                if let Some(rotation) = stream.metadata().get("rotate") {
                    item.rotation = rotation.parse().unwrap_or(0.0);
                }
                for side in stream.side_data() {
                    if side.kind() == ffmpeg::codec::packet::side_data::Type::DisplayMatrix
                        && side.data().len() >= 36
                    {
                        let mut matrix = [0_i32; 9];
                        for (i, chunk) in side.data()[..36].chunks_exact(4).enumerate() {
                            matrix[i] =
                                i32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        }
                        item.rotation =
                            -unsafe { ffmpeg::ffi::av_display_rotation_get(matrix.as_ptr()) };
                    }
                }
                if video_index.is_none() {
                    video_index = Some(stream.index());
                    video_base = f64::from(stream.time_base());
                }
            }
            ffmpeg::media::Type::Audio => {
                let decoder = cli_try!(context.decoder().audio(), "unreadable_media", "", 4);
                item.kind = "audio".into();
                item.sample_rate = decoder.rate();
                item.channels = decoder.channels();
            }
            _ => {}
        }
        streams.push(item);
    }
    let mut first = None;
    let mut last = None;
    let mut intervals = 0;
    for (stream, packet) in input.packets().take(4096) {
        if Some(stream.index()) != video_index || !packet.is_key() {
            continue;
        }
        let Some(pts) = packet.pts() else {
            continue;
        };
        if first.is_none() {
            first = Some(pts);
        } else {
            intervals += 1;
        }
        last = Some(pts);
        if intervals >= 16 {
            break;
        }
    }
    let keyframe_interval_s = if intervals > 0 {
        Some((last.unwrap() - first.unwrap()) as f64 * video_base / intervals as f64)
    } else {
        None
    };
    if streams.is_empty() {
        return Err(cli_error!(
            "unreadable_media",
            "",
            4,
            "no streams in {}",
            path.display()
        ));
    }
    Ok(Probe {
        container: input.format().name().into(),
        duration,
        streams,
        keyframe_interval_s,
    })
}

pub fn assets(doc: &Document, base: &Path) -> Result<HashMap<ulid::Ulid, MediaInfo>> {
    let (infos, findings) = inspect_assets(doc, base);
    if let Some(finding) = findings.into_iter().next() {
        return Err(finding.error);
    }
    Ok(infos)
}

pub fn inspect_assets(
    doc: &Document,
    base: &Path,
) -> (
    HashMap<ulid::Ulid, MediaInfo>,
    Vec<crate::cli::validate::Finding>,
) {
    let mut infos = HashMap::new();
    let mut findings = Vec::new();
    for (i, asset) in doc.assets.iter().enumerate() {
        let path = base.join(&asset.path);
        let p = match probe(&path) {
            Ok(p) => p,
            Err(mut e) => {
                e.pointer = format!("/assets/{i}/path");
                e.message = format!("{}: {}", path.display(), e.message);
                findings.push(crate::cli::validate::Finding {
                    error: e,
                    fix_hint: None,
                });
                continue;
            }
        };
        infos.insert(
            asset.id,
            MediaInfo {
                duration: p.duration,
                video: p.streams.iter().any(|s| s.kind == "video"),
                audio: p.streams.iter().any(|s| s.kind == "audio"),
                image: is_image(&path),
                video_bitrate: p
                    .streams
                    .iter()
                    .find(|s| s.kind == "video")
                    .and_then(|s| s.bitrate),
            },
        );
    }
    (infos, findings)
}
