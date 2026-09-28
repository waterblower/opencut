use ::transcribe::audio::extract_audio_as_wav;
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use ulid::Ulid;
use {
    engine::encode::{Encoder, VideoEncoding},
    opencut::{document, transcribe},
    timeline::FrameRate,
};

#[test]
fn normalizes_stereo_audio_to_mono_16khz_and_drains_resampler() {
    let temp = Temp::new();
    for rate in [8_000, 44_100, 48_000] {
        let input = temp.0.join(format!("{rate}.wav"));
        write_wav(&input, rate, rate);
        let wav = extract_audio_as_wav(&input).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16_000);
        assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()), 16);
        assert_eq!(wav.len(), 44 + 16_000 * 2, "rate {rate}");
        let sample = i16::from_le_bytes(wav[20_044..20_046].try_into().unwrap());
        assert!((sample as i32 - 16_384).abs() <= 2, "sample {sample}");
    }
}

#[test]
fn extracts_video_audio_preserving_initial_offset_and_gaps() {
    let temp = Temp::new();
    let input = temp.0.join("recording.mov");
    write_video(&input, true);
    let wav = extract_audio_as_wav(&input).unwrap();
    let samples: Vec<_> = wav[44..]
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();
    assert!(
        samples.len() >= 15_900 && samples.len() <= 16_500,
        "{} samples",
        samples.len()
    );
    assert!(samples[..3_000].iter().all(|s| *s == 0));
    assert!(samples[4_800..6_400].iter().any(|s| s.abs() > 100));
    assert!(samples[9_600..11_000].iter().all(|s| *s == 0));
    assert!(samples[12_800..14_400].iter().any(|s| s.abs() > 100));
}

#[tokio::test]
async fn extracts_whole_audio_and_transcription_rejects_overlong_input() {
    let temp = Temp::new();
    let silent_video = temp.0.join("no-audio.mov");
    write_video(&silent_video, false);
    assert!(
        extract_audio_as_wav(&silent_video)
            .unwrap_err()
            .to_string()
            .contains("missing_audio")
    );
    let long = temp.0.join("too-long.wav");
    write_wav(&long, 8_000, 500 * 8_000);
    assert_eq!(
        extract_audio_as_wav(&long).unwrap().len(),
        44 + 500 * 16_000 * 2
    );
    write_wav(&long, 16_000, 500 * 16_000 + 1);
    assert_eq!(
        extract_audio_as_wav(&long).unwrap().len(),
        44 + (500 * 16_000 + 1) * 2
    );
    let error = ::transcribe::transcribe(&long, "test-key", &::transcribe::Options::default())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("audio_too_long"));
    assert!(error.to_string().contains("500.000063 seconds"));
    assert!(error.to_string().contains("at most 500 seconds"));
    assert!(
        extract_audio_as_wav(&temp.0.join("missing.wav"))
            .unwrap_err()
            .to_string()
            .contains("unreadable_media")
    );
}

#[tokio::test]
async fn async_publication_preserves_bytes_and_protects_input_and_existing_output() {
    let temp = Temp::new();
    let input = temp.0.join("input.wav");
    fs::write(&input, b"original media").unwrap();
    let output = temp.0.join("transcript.srt");
    transcribe::check_output(&input, &output, false)
        .await
        .unwrap();
    let text = "1\r\n00:00:00,000 --> 00:00:01,000\r\n你好\r\n\r\n";
    document::write_atomic_bytes(&output, text.as_bytes().to_vec(), false)
        .await
        .unwrap();
    assert_eq!(fs::read(&output).unwrap(), text.as_bytes());
    assert!(
        transcribe::check_output(&input, &output, false)
            .await
            .unwrap_err()
            .to_string()
            .contains("output_exists")
    );
    assert!(
        transcribe::check_output(&input, &input, true)
            .await
            .unwrap_err()
            .to_string()
            .contains("output_is_source")
    );
    // Publication must also refuse a destination that appeared after preflight.
    assert!(
        document::write_atomic_bytes(&output, b"new".to_vec(), false)
            .await
            .is_err()
    );
    assert_eq!(fs::read(&output).unwrap(), text.as_bytes());
    transcribe::check_output(&input, &output, true)
        .await
        .unwrap();
    document::write_atomic_bytes(&output, b"new".to_vec(), true)
        .await
        .unwrap();
    assert_eq!(fs::read(&output).unwrap(), b"new");
    assert_eq!(fs::read(&input).unwrap(), b"original media");
    assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 2);
}

#[test]
fn cli_validates_flags_credentials_and_output_before_contacting_minimax() {
    let temp = Temp::new();
    let input = temp.0.join("input.wav");
    write_wav(&input, 16_000, 16_000);
    for (arguments, expected, exit) in [
        (vec!["--format", "invalid"], "usage_error", 2),
        (vec!["--timestamp-level", "invalid"], "usage_error", 2),
        (vec!["--post-merge"], "usage_error", 1),
        (vec!["--format", "vtt", "--post-merge"], "usage_error", 1),
        (
            vec!["--format", "srt", "--post-merge"],
            "missing_api_key",
            1,
        ),
        (vec![], "missing_api_key", 1),
        (
            vec!["-o", input.to_str().unwrap(), "--overwrite"],
            "output_is_source",
            1,
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_opencut"))
            .arg("transcribe")
            .arg(&input)
            .args(&arguments)
            .arg("--json")
            .env_remove("MINIMAX_API_KEY")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(exit));
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains(expected)
        );
        assert!(value["error"]["message"].as_str().unwrap().contains(" at "));
    }
    let output = Command::new(env!("CARGO_BIN_EXE_opencut"))
        .args(["transcribe", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("verbose_json") && !help.contains("--timestamp-level"));
    // JSON document output remains compatible with the synchronous caller.
    let path = temp.0.join("document.json");
    let value = json!({"text":"hello"});
    document::write_atomic(&path, &value, false).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap(),
        value
    );
}

fn write_wav(path: &Path, rate: u32, frames: u32) {
    let size = frames * 4;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(size + 36).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 4).to_le_bytes());
    wav.extend_from_slice(&4_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&size.to_le_bytes());
    for _ in 0..frames {
        wav.extend_from_slice(&8192_i16.to_le_bytes());
        wav.extend_from_slice(&24576_i16.to_le_bytes());
    }
    fs::write(path, wav).unwrap();
}

fn write_video(path: &Path, audio: bool) {
    let video_path = path.with_extension("video.mov");
    let mut encoder = Encoder::open(
        &video_path,
        (64, 48),
        FrameRate::default(),
        &VideoEncoding {
            codec: "prores".into(),
            preset: "draft".into(),
            bitrate: 128_000,
        },
    )
    .unwrap();
    for _ in 0..30 {
        encoder
            .encode_new_frame(&RgbaImage::from_pixel(64, 48, Rgba([0, 0, 0, 255])))
            .unwrap();
    }
    encoder.finish().unwrap();
    if !audio {
        fs::rename(&video_path, path).unwrap();
        return;
    }

    // Encode this test fixture directly so audio offset and gap coverage does
    // not require unused production audio encoding APIs.
    let mut input = ffmpeg_next::format::input(&video_path).unwrap();
    let mut output = ffmpeg_next::format::output(path).unwrap();
    let video = input
        .streams()
        .best(ffmpeg_next::media::Type::Video)
        .unwrap();
    let video_base = video.time_base();
    {
        let mut stream = output
            .add_stream(ffmpeg_next::encoder::find(ffmpeg_next::codec::Id::PRORES))
            .unwrap();
        stream.set_parameters(video.parameters());
        stream.set_time_base(video_base);
    }
    let aac = ffmpeg_next::encoder::find(ffmpeg_next::codec::Id::AAC).unwrap();
    let mut audio = ffmpeg_next::codec::context::Context::new_with_codec(aac)
        .encoder()
        .audio()
        .unwrap();
    audio.set_rate(48_000);
    audio.set_channel_layout(ffmpeg_next::ChannelLayout::STEREO);
    audio.set_format(ffmpeg_next::format::Sample::F32(
        ffmpeg_next::format::sample::Type::Planar,
    ));
    audio.set_time_base((1, 48_000));
    audio.set_bit_rate(192_000);
    audio.set_flags(ffmpeg_next::codec::Flags::GLOBAL_HEADER);
    let mut audio = audio.open_as(aac).unwrap();
    {
        let mut stream = output.add_stream(aac).unwrap();
        stream.set_parameters(&audio);
        stream.set_time_base((1, 48_000));
    }
    output.write_header().unwrap();
    let output_video_base = output.stream(0).unwrap().time_base();
    for (_, mut packet) in input.packets() {
        packet.set_stream(0);
        packet.set_position(-1);
        packet.rescale_ts(video_base, output_video_base);
        packet.write_interleaved(&mut output).unwrap();
    }
    for (from, to) in [(12_288, 24_576), (36_864, 49_152)] {
        for start in (from..to).step_by(audio.frame_size() as usize) {
            let count = (audio.frame_size() as i64).min(to - start) as usize;
            let mut frame = ffmpeg_next::frame::Audio::new(
                audio.format(),
                count,
                ffmpeg_next::ChannelLayout::STEREO,
            );
            frame.set_rate(48_000);
            frame.set_pts(Some(start));
            for channel in 0..2 {
                frame.plane_mut::<f32>(channel).fill(0.25);
            }
            audio.send_frame(&frame).unwrap();
            drain_fixture_audio(&mut audio, &mut output);
        }
    }
    audio.send_eof().unwrap();
    drain_fixture_audio(&mut audio, &mut output);
    output.write_trailer().unwrap();
    drop(input);
    fs::remove_file(video_path).unwrap();
}

fn drain_fixture_audio(
    audio: &mut ffmpeg_next::encoder::Audio,
    output: &mut ffmpeg_next::format::context::Output,
) {
    loop {
        let mut packet = ffmpeg_next::Packet::empty();
        match audio.receive_packet(&mut packet) {
            Ok(()) => {
                packet.set_stream(1);
                packet.rescale_ts((1, 48_000), output.stream(1).unwrap().time_base());
                packet.write_interleaved(output).unwrap();
            }
            Err(ffmpeg_next::Error::Eof) => return,
            Err(ffmpeg_next::Error::Other { errno }) if errno == ffmpeg_next::error::EAGAIN => {
                return;
            }
            Err(error) => panic!("encoding fixture audio: {error:?}"),
        }
    }
}

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("opencut-transcribe-{}", Ulid::generate()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
