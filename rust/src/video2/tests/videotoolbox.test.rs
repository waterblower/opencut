use super::*;

#[test]
fn hevc_nal_parser_distinguishes_leading_pictures_and_rejects_truncation() -> Result<()> {
    for kind in [1, 8, 9, 19, 21] {
        let packet = [0, 0, 0, 2, 64, 1, 0, 0, 0, 2, kind << 1, 1];
        assert_eq!(hevc_picture_type(&packet, 4)?, Some(kind));
    }
    assert_eq!(hevc_picture_type(&[2, 16, 1], 1)?, Some(8));
    assert_eq!(hevc_picture_type(&[0, 2, 18, 1], 2)?, Some(9));
    assert!(hevc_picture_type(&[0, 0], 4).is_err());
    assert!(hevc_picture_type(&[0, 0, 0, 9, 2], 4).is_err());
    assert!(hevc_picture_type(&[0, 0, 0, 0], 4).is_err());
    assert_eq!(hevc_picture_type(&[0, 0, 0, 2, 64, 1], 4)?, None);
    Ok(())
}

#[test]
fn unsupported_codec_stays_on_the_software_path() -> Result<()> {
    assert!(
        Decoder::open(&ffmpeg::codec::Parameters::new(), ffmpeg::Rational(1, 1000))?.is_none()
    );
    Ok(())
}
