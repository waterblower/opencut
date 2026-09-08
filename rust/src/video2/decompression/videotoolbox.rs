//! Bounded asynchronous VideoToolbox decoding of length-prefixed AVC/HEVC.
//! Compressed packet owners survive the completion barrier. Output planes are
//! retained and mapped read-only, never copied during decoding.

use std::{
    collections::BTreeMap,
    ffi::c_void,
    ptr,
    sync::{Arc, mpsc},
};

use anyhow::{Context as _, Result, bail};
use core_foundation::{
    base::{CFType, TCFType, kCFAllocatorNull},
    boolean::CFBoolean,
    data::CFData,
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};
use core_video::pixel_buffer::{CVPixelBuffer, CVPixelBufferKeys, CVPixelBufferRef};
use ffmpeg_next as ffmpeg;

pub struct Decoder {
    pub seek: Option<i64>,
    session: CFType,
    format: CFType,
    // The callback address must stay stable even when Decoder moves.
    callback: Box<mpsc::Sender<Event>>,
    events: mpsc::Receiver<Event>,
    pending: Vec<ffmpeg::Packet>,
    ready: BTreeMap<i64, (Surface, i64)>,
    parameters: ffmpeg::codec::Parameters,
    base: ffmpeg::Rational,
    watermark: i64,
    ended: bool,
    reset: bool,
    skip_rasl: bool,
}

impl Decoder {
    pub fn open(
        parameters: &ffmpeg::codec::Parameters,
        base: ffmpeg::Rational,
    ) -> Result<Option<Self>> {
        // SAFETY: parameters is borrowed for this call; extradata is read only
        // and copied into a CFData before its owner can be dropped.
        let params = unsafe { &*parameters.as_ptr() };
        let (codec, atom) = match parameters.id() {
            ffmpeg::codec::Id::HEVC => (u32::from_be_bytes(*b"hvc1"), "hvcC"),
            ffmpeg::codec::Id::H264 => (u32::from_be_bytes(*b"avc1"), "avcC"),
            _ => return Ok(None),
        };
        // Preserve the software path for unsupported layouts and Annex B.
        if params.format != ffmpeg::ffi::AVPixelFormat::AV_PIX_FMT_YUV420P as i32
            || params.extradata.is_null()
            || params.extradata_size < 7
            || base.numerator() <= 0
            || base.denominator() <= 0
        {
            return Ok(None);
        }
        let extra =
            unsafe { std::slice::from_raw_parts(params.extradata, params.extradata_size as usize) };
        if extra[0] != 1 {
            return Ok(None);
        }
        let atoms = CFDictionary::from_CFType_pairs(&[(
            CFString::new(atom),
            CFData::from_buffer(extra).as_CFType(),
        )]);
        let extensions = CFDictionary::from_CFType_pairs(&[(
            unsafe {
                CFString::wrap_under_get_rule(
                    kCMFormatDescriptionExtension_SampleDescriptionExtensionAtoms.cast(),
                )
            },
            atoms.as_CFType(),
        )]);
        let mut raw_format = ptr::null();
        // SAFETY: valid dimensions/configuration, live CF dictionaries, writable output.
        let status = unsafe {
            CMVideoFormatDescriptionCreate(
                ptr::null(),
                codec,
                params.width,
                params.height,
                extensions.as_concrete_TypeRef().cast(),
                &mut raw_format,
            )
        };
        check(status, "Creating hardware video format")?;
        let format = unsafe { CFType::wrap_under_create_rule(raw_format) };
        let specification = CFDictionary::from_CFType_pairs(&[(
            unsafe {
                CFString::wrap_under_get_rule(
                    kVTVideoDecoderSpecification_RequireHardwareAcceleratedVideoDecoder.cast(),
                )
            },
            CFBoolean::true_value().as_CFType(),
        )]);
        let pixel_format = if params.color_range == ffmpeg::ffi::AVColorRange::AVCOL_RANGE_JPEG {
            u32::from_be_bytes(*b"420f")
        } else {
            u32::from_be_bytes(*b"420v")
        };
        let attributes = CFDictionary::from_CFType_pairs(&[
            (
                CFString::from(CVPixelBufferKeys::PixelFormatType),
                CFNumber::from(pixel_format as i32).as_CFType(),
            ),
            (
                CFString::from(CVPixelBufferKeys::IOSurfaceProperties),
                CFDictionary::<CFString, CFType>::from_CFType_pairs(&[]).as_CFType(),
            ),
            (
                CFString::from(CVPixelBufferKeys::MetalCompatibility),
                CFBoolean::true_value().as_CFType(),
            ),
        ]);
        let (sender, events) = mpsc::channel();
        let mut callback = Box::new(sender);
        let record = Callback {
            function: output,
            context: (&mut *callback as *mut mpsc::Sender<Event>).cast(),
        };
        let mut session = ptr::null();
        // SAFETY: the callback owner has a stable allocation and outlives the
        // session. Apple retains format/attributes needed after this call.
        let status = unsafe {
            VTDecompressionSessionCreate(
                ptr::null(),
                format.as_CFTypeRef(),
                specification.as_concrete_TypeRef().cast(),
                attributes.as_concrete_TypeRef().cast(),
                &record,
                &mut session,
            )
        };
        if matches!(status, -12906 | -12910 | -12913) {
            return Ok(None);
        }
        check(status, "Creating hardware decoder")?;
        let decoder = Self {
            seek: None,
            session: unsafe { CFType::wrap_under_create_rule(session) },
            format,
            callback,
            events,
            pending: Vec::with_capacity(BATCH),
            ready: BTreeMap::new(),
            parameters: parameters.clone(),
            base,
            watermark: i64::MIN,
            ended: false,
            reset: true,
            skip_rasl: true,
        };
        // Seeking needs maximum throughput, not a real-time/low-power hint.
        // This optional property is not implemented by every hardware decoder.
        let status = unsafe {
            VTSessionSetProperty(
                decoder.session.as_CFTypeRef(),
                kVTDecompressionPropertyKey_RealTime,
                CFBoolean::false_value().as_CFTypeRef(),
            )
        };
        if status != -12900 {
            check(status, "Configuring hardware decode throughput")?;
        }
        Ok(Some(decoder))
    }

    pub fn send_packet(&mut self, packet: &ffmpeg::Packet) -> Result<()> {
        if self.ended || self.pending.len() >= SEEK_BATCH {
            bail!(
                "Hardware decoder input not drained at {}:{}",
                file!(),
                line!()
            );
        }
        let pts = packet.pts().context(format!(
            "Hardware packet lacks PTS at {}:{}",
            file!(),
            line!()
        ))?;
        let dts = packet.dts().context(format!(
            "Hardware packet lacks DTS at {}:{}",
            file!(),
            line!()
        ))?;
        let previous_dts = self
            .pending
            .last()
            .and_then(ffmpeg::Packet::dts)
            .unwrap_or(self.watermark);
        if pts < dts || dts < previous_dts {
            bail!(
                "Unsupported hardware packet timestamp ordering at {}:{}",
                file!(),
                line!()
            );
        }
        if self.skip_rasl && self.parameters.id() == ffmpeg::codec::Id::HEVC {
            // HEVC random access skips RASL leading pictures: they can refer
            // to pictures before the CRA seek point and are not needed by the
            // following pictures. FFmpeg's HEVC decoder does the same.
            let params = unsafe { &*self.parameters.as_ptr() };
            if params.extradata_size < 23 {
                bail!("Truncated HEVC configuration at {}:{}", file!(), line!());
            }
            let length = unsafe { *params.extradata.add(21) & 3 } as usize + 1;
            let bytes =
                packet
                    .data()
                    .context(format!("Empty HEVC packet at {}:{}", file!(), line!()))?;
            match hevc_picture_type(bytes, length)? {
                Some(8 | 9) => return Ok(()),
                Some(0..=15) => self.skip_rasl = false,
                _ => {}
            }
        }
        let bytes =
            packet
                .data()
                .context(format!("Empty hardware packet at {}:{}", file!(), line!()))?;
        if bytes.len() > MAX_PACKET_BYTES {
            bail!(
                "Hardware packet exceeds memory limit at {}:{}",
                file!(),
                line!()
            );
        }
        let pending_bytes: usize = self.pending.iter().map(ffmpeg::Packet::size).sum();
        if pending_bytes + bytes.len() > MAX_PENDING_BYTES {
            self.complete()?;
        }
        self.pending.push(packet.clone());
        Ok(())
    }

    pub fn receive(&mut self) -> Result<Option<(ffmpeg::frame::Video, bool)>> {
        let batch = if self.seek.is_some() {
            SEEK_BATCH
        } else {
            BATCH
        };
        let reached = match (self.seek, self.pending.last().and_then(ffmpeg::Packet::dts)) {
            (Some(target), Some(dts)) => dts > target,
            _ => false,
        };
        if self.pending.len() >= batch || reached {
            self.complete()?;
        }
        let Some((&pts, _)) = self.ready.first_key_value() else {
            return Ok(None);
        };
        // DTS is a lower bound on future presentation timestamps. Do not emit
        // reordered pictures until every earlier decode packet has completed.
        if !self.ended && pts > self.watermark {
            return Ok(None);
        }
        let Some((_, (surface, duration))) = self.ready.pop_first() else {
            return Ok(None);
        };
        Ok(Some((
            surface.frame(pts, duration, &self.parameters)?,
            self.seek.is_none(),
        )))
    }

    pub fn send_eof(&mut self) -> Result<()> {
        self.complete()?;
        self.ended = true;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        // Pending packets have not been submitted; completed batches already
        // crossed their barrier. A seek can discard this compressed lookahead.
        self.pending.clear();
        self.ready.clear();
        self.watermark = i64::MIN;
        self.ended = false;
        self.reset = true;
        self.skip_rasl = true;
        self.seek = None;
        Ok(())
    }

    fn submit(&mut self, packet: &ffmpeg::Packet, emit: bool) -> Result<()> {
        let pts = packet.pts().context(format!(
            "Hardware packet lacks PTS at {}:{}",
            file!(),
            line!()
        ))?;
        let dts = packet.dts().context(format!(
            "Hardware packet lacks DTS at {}:{}",
            file!(),
            line!()
        ))?;
        let timing = Timing {
            duration: time(packet.duration(), self.base)?,
            pts: time(pts, self.base)?,
            dts: time(dts, self.base)?,
        };
        let packet = packet.clone();
        let bytes =
            packet
                .data()
                .context(format!("Empty hardware packet at {}:{}", file!(), line!()))?;
        if bytes.len() > MAX_PACKET_BYTES {
            bail!(
                "Hardware packet exceeds memory limit at {}:{}",
                file!(),
                line!()
            );
        }
        let mut raw_block = ptr::null();
        // SAFETY: CM borrows the refcounted packet's immutable allocation, using
        // the null allocator (never free it). pending retains it through wait.
        let status = unsafe {
            CMBlockBufferCreateWithMemoryBlock(
                ptr::null(),
                bytes.as_ptr().cast_mut().cast(),
                bytes.len(),
                kCFAllocatorNull,
                ptr::null(),
                0,
                bytes.len(),
                0,
                &mut raw_block,
            )
        };
        check(status, "Wrapping compressed video packet")?;
        let block = unsafe { CFType::wrap_under_create_rule(raw_block) };
        let size = bytes.len();
        let mut raw_sample = ptr::null();
        let status = unsafe {
            CMSampleBufferCreateReady(
                ptr::null(),
                block.as_CFTypeRef(),
                self.format.as_CFTypeRef(),
                1,
                1,
                &timing,
                1,
                &size,
                &mut raw_sample,
            )
        };
        check(status, "Creating compressed video sample")?;
        let sample = unsafe { CFType::wrap_under_create_rule(raw_sample) };
        if self.reset {
            // Reset reference pictures at discontinuities, after the previous
            // batch has finished. This is a buffer attachment, not a sample key.
            unsafe {
                CMSetAttachment(
                    sample.as_CFTypeRef(),
                    kCMSampleBufferAttachmentKey_ResetDecoderBeforeDecoding,
                    CFBoolean::true_value().as_CFTypeRef(),
                    1,
                )
            };
            self.reset = false;
        }
        // Enable asynchronous decompression; do NOT wait after each packet.
        let status = unsafe {
            VTDecompressionSessionDecodeFrame(
                self.session.as_CFTypeRef(),
                sample.as_CFTypeRef(),
                1 | if emit { 0 } else { 2 },
                // Opaque marker only; never dereferenced by us or VideoToolbox.
                if emit {
                    ptr::dangling_mut()
                } else {
                    ptr::null_mut()
                },
                ptr::null_mut(),
            )
        };
        check(status, "Submitting hardware video packet")
    }

    fn complete(&mut self) -> Result<()> {
        // Keep only the greatest submitted PTS at/before the target in each
        // batch. Every other preroll picture is still decoded as a dependency.
        let selected = self
            .pending
            .iter()
            .filter_map(ffmpeg::Packet::pts)
            .filter(|pts| self.seek.is_some_and(|target| *pts <= target))
            .max();
        for index in 0..self.pending.len() {
            let packet = self.pending[index].clone();
            let emit = match (self.seek, packet.pts()) {
                (Some(target), Some(pts)) if pts <= target => Some(pts) == selected,
                _ => true,
            };
            self.submit(&packet, emit)?;
        }
        if self.pending.is_empty() {
            return Ok(());
        }
        // This barrier also finishes delayed output. No borrowed packet or
        // callback owner can be released while the hardware still uses it.
        let status =
            unsafe { VTDecompressionSessionWaitForAsynchronousFrames(self.session.as_CFTypeRef()) };
        check(status, "Waiting for hardware batch")?;
        if let Some(packet) = self.pending.last() {
            self.watermark = packet.dts().context(format!(
                "Completed hardware packet lacks DTS at {}:{}",
                file!(),
                line!()
            ))?;
        }
        self.pending.clear();
        while let Ok(event) = self.events.try_recv() {
            check(event.status, "Receiving hardware video frame")?;
            let Some(surface) = event.surface else {
                if event.expected {
                    bail!(
                        "Hardware decoder dropped a requested picture at {}:{}",
                        file!(),
                        line!()
                    );
                }
                continue;
            };
            if event.pts.flags & 1 == 0 || event.pts.scale <= 0 {
                bail!("Hardware frame lacks timestamp at {}:{}", file!(), line!());
            }
            let pts = unsafe {
                ffmpeg::ffi::av_rescale_q(
                    event.pts.value,
                    ffmpeg::Rational(1, event.pts.scale).into(),
                    self.base.into(),
                )
            };
            let duration = if event.duration.flags & 1 != 0 && event.duration.scale > 0 {
                unsafe {
                    ffmpeg::ffi::av_rescale_q(
                        event.duration.value,
                        ffmpeg::Rational(1, event.duration.scale).into(),
                        self.base.into(),
                    )
                }
            } else {
                0
            };
            self.ready.insert(pts, (surface, duration));
        }
        if self.ready.len() > MAX_REORDERED {
            bail!(
                "Hardware reorder queue exceeds limit at {}:{}",
                file!(),
                line!()
            );
        }
        Ok(())
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: wait/invalidate before pending packets and callback are freed.
        unsafe {
            VTDecompressionSessionWaitForAsynchronousFrames(self.session.as_CFTypeRef());
            VTDecompressionSessionInvalidate(self.session.as_CFTypeRef());
        }
        // Keep the boxed callback owner visibly live through invalidation.
        let _ = &self.callback;
    }
}

const BATCH: usize = 16;
const SEEK_BATCH: usize = 64;
const MAX_REORDERED: usize = 64;
const MAX_PACKET_BYTES: usize = 4 * 1024 * 1024;
const MAX_PENDING_BYTES: usize = 8 * 1024 * 1024;
const READ_ONLY: u64 = 1;

struct Surface(CVPixelBuffer);
// CoreVideo buffers are reference-counted; callback output is immutable here.
// Only a read-only base-address lock is taken, never mutable access.
unsafe impl Send for Surface {}
unsafe impl Sync for Surface {}

struct LockedSurface(Surface);
impl Drop for LockedSurface {
    fn drop(&mut self) {
        self.0.0.unlock_base_address(READ_ONLY);
    }
}

impl Surface {
    fn frame(
        self,
        pts: i64,
        duration: i64,
        parameters: &ffmpeg::codec::Parameters,
    ) -> Result<ffmpeg::frame::Video> {
        let buffer = &self.0;
        let format = buffer.get_pixel_format();
        if !matches!(format, 0x34323076 | 0x34323066) || buffer.get_plane_count() != 2 {
            bail!("Hardware output is not NV12 at {}:{}", file!(), line!());
        }
        check(
            buffer.lock_base_address(READ_ONLY),
            "Mapping hardware video planes",
        )?;
        let owner = Arc::new(LockedSurface(self));
        let buffer = &owner.0.0;
        let mut frame = ffmpeg::frame::Video::empty();
        frame.set_format(ffmpeg::format::Pixel::NV12);
        frame.set_width(buffer.get_width() as u32);
        frame.set_height(buffer.get_height() as u32);
        // SAFETY: frame is exclusively owned. Each plane gets its own AVBuffer
        // with a shared owner, keeping the CVPixelBuffer locked until all FFmpeg
        // references are gone. READONLY prevents FFmpeg from mutating hardware.
        unsafe {
            let params = &*parameters.as_ptr();
            let raw = &mut *frame.as_mut_ptr();
            raw.pts = pts;
            raw.best_effort_timestamp = pts;
            raw.duration = duration;
            raw.colorspace = params.color_space;
            raw.color_primaries = params.color_primaries;
            raw.color_trc = params.color_trc;
            raw.color_range = if format == 0x34323066 {
                ffmpeg::ffi::AVColorRange::AVCOL_RANGE_JPEG
            } else {
                ffmpeg::ffi::AVColorRange::AVCOL_RANGE_MPEG
            };
            raw.chroma_location = params.chroma_location;
            for plane in 0..2 {
                let stride = buffer.get_bytes_per_row_of_plane(plane);
                let bytes = stride
                    .checked_mul(buffer.get_height_of_plane(plane))
                    .context(format!(
                        "Hardware plane size overflow at {}:{}",
                        file!(),
                        line!()
                    ))?;
                let data = buffer.get_base_address_of_plane(plane).cast::<u8>();
                if data.is_null() || stride > i32::MAX as usize || bytes == 0 {
                    bail!("Invalid hardware plane at {}:{}", file!(), line!());
                }
                let retained = Box::into_raw(Box::new(Arc::clone(&owner))).cast();
                let reference = ffmpeg::ffi::av_buffer_create(
                    data,
                    bytes,
                    Some(release_plane),
                    retained,
                    ffmpeg::ffi::AV_BUFFER_FLAG_READONLY,
                );
                if reference.is_null() {
                    drop(Box::from_raw(retained.cast::<Arc<LockedSurface>>()));
                    bail!("Retaining hardware plane at {}:{}", file!(), line!());
                }
                raw.buf[plane] = reference;
                raw.data[plane] = data;
                raw.linesize[plane] = stride as i32;
            }
        }
        Ok(frame)
    }
}

unsafe extern "C" fn release_plane(opaque: *mut c_void, _: *mut u8) {
    // SAFETY: this box is transferred exactly once to av_buffer_create.
    unsafe {
        drop(Box::from_raw(opaque.cast::<Arc<LockedSurface>>()));
    }
}

struct Event {
    status: i32,
    expected: bool,
    surface: Option<Surface>,
    pts: Time,
    duration: Time,
}

unsafe extern "C" fn output(
    context: *mut c_void,
    source: *mut c_void,
    status: i32,
    _: u32,
    image: CVPixelBufferRef,
    pts: Time,
    duration: Time,
) {
    // SAFETY: context outlives the session; retain callback-owned image before
    // returning. Sending is nonblocking and no Rust panic crosses this callback.
    let sender = unsafe { &*context.cast::<mpsc::Sender<Event>>() };
    let surface = if image.is_null() {
        None
    } else {
        Some(Surface(unsafe {
            CVPixelBuffer::wrap_under_get_rule(image)
        }))
    };
    let _ = sender.send(Event {
        status,
        expected: !source.is_null(),
        surface,
        pts,
        duration,
    });
}

fn check(status: i32, operation: &str) -> Result<()> {
    if status != 0 {
        bail!("{operation}: OSStatus {status} at {}:{}", file!(), line!());
    }
    Ok(())
}

fn hevc_picture_type(mut bytes: &[u8], length: usize) -> Result<Option<u8>> {
    while !bytes.is_empty() {
        let prefix = bytes.get(..length).context(format!(
            "Truncated HEVC NAL length at {}:{}",
            file!(),
            line!()
        ))?;
        let mut size = 0usize;
        for byte in prefix {
            size = (size << 8) | usize::from(*byte);
        }
        bytes = &bytes[length..];
        let nal =
            bytes
                .get(..size)
                .context(format!("Truncated HEVC NAL at {}:{}", file!(), line!()))?;
        if nal.len() < 2 {
            bail!("Invalid HEVC NAL header at {}:{}", file!(), line!());
        }
        let kind = (nal[0] >> 1) & 63;
        if kind < 32 {
            return Ok(Some(kind));
        }
        bytes = &bytes[size..];
    }
    Ok(None)
}

fn time(value: i64, base: ffmpeg::Rational) -> Result<Time> {
    Ok(Time {
        value: value
            .checked_mul(i64::from(base.numerator()))
            .context(format!(
                "Hardware timestamp overflow at {}:{}",
                file!(),
                line!()
            ))?,
        scale: base.denominator(),
        flags: 1,
        epoch: 0,
    })
}

type Ref = *const c_void;
#[repr(C)]
#[derive(Clone, Copy)]
struct Time {
    value: i64,
    scale: i32,
    flags: u32,
    epoch: i64,
}
#[repr(C)]
struct Timing {
    duration: Time,
    pts: Time,
    dts: Time,
}
#[repr(C)]
struct Callback {
    function:
        unsafe extern "C" fn(*mut c_void, *mut c_void, i32, u32, CVPixelBufferRef, Time, Time),
    context: *mut c_void,
}

#[link(name = "CoreMedia", kind = "framework")]
unsafe extern "C" {
    static kCMFormatDescriptionExtension_SampleDescriptionExtensionAtoms: Ref;
    static kCMSampleBufferAttachmentKey_ResetDecoderBeforeDecoding: Ref;
    fn CMVideoFormatDescriptionCreate(
        allocator: Ref,
        codec: u32,
        width: i32,
        height: i32,
        extensions: Ref,
        output: *mut Ref,
    ) -> i32;
    fn CMBlockBufferCreateWithMemoryBlock(
        allocator: Ref,
        memory: *mut c_void,
        length: usize,
        block_allocator: Ref,
        custom: Ref,
        offset: usize,
        size: usize,
        flags: u32,
        output: *mut Ref,
    ) -> i32;
    fn CMSampleBufferCreateReady(
        allocator: Ref,
        data: Ref,
        format: Ref,
        samples: isize,
        timings: isize,
        timing: *const Timing,
        sizes: isize,
        size: *const usize,
        output: *mut Ref,
    ) -> i32;
    fn CMSetAttachment(buffer: Ref, key: Ref, value: Ref, mode: u32);
}
#[link(name = "VideoToolbox", kind = "framework")]
unsafe extern "C" {
    static kVTVideoDecoderSpecification_RequireHardwareAcceleratedVideoDecoder: Ref;
    static kVTDecompressionPropertyKey_RealTime: Ref;
    fn VTSessionSetProperty(session: Ref, key: Ref, value: Ref) -> i32;
    fn VTDecompressionSessionCreate(
        allocator: Ref,
        format: Ref,
        specification: Ref,
        attributes: Ref,
        callback: *const Callback,
        output: *mut Ref,
    ) -> i32;
    fn VTDecompressionSessionDecodeFrame(
        session: Ref,
        sample: Ref,
        flags: u32,
        frame: *mut c_void,
        info: *mut u32,
    ) -> i32;
    fn VTDecompressionSessionWaitForAsynchronousFrames(session: Ref) -> i32;
    fn VTDecompressionSessionInvalidate(session: Ref);
}

#[cfg(test)]
mod tests {
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
}
