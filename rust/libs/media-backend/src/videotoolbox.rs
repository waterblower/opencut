//! Pipelined VideoToolbox decoding for H.264 and HEVC, the macOS video decoder.
//!
//! FFmpeg's VideoToolbox hwaccel waits for every frame before sending the
//! next one. Driving the session directly keeps several frames in flight, which
//! decodes a group of pictures several times faster after a seek. FFmpeg still
//! demuxes; packets go to VideoToolbox unchanged. Streams outside MP4-style
//! H.264/HEVC in 4:2:0 at 8 or 10 bits are errors.

use anyhow::{Result, bail};
use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::data::CFData;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};
use core_video::pixel_buffer::{CVPixelBuffer, CVPixelBufferRef};
use ffmpeg_next::{Packet, Rational, ffi, util::color};
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::{Arc, Condvar, Mutex};

/// A decoded picture in stream time-base ticks.
pub struct DecodedPicture {
    pub pixel_buffer: CVPixelBuffer,
    pub pts: i64,
    pub duration: i64,
    pub color_space: color::Space,
}

pub enum Received {
    Picture(DecodedPicture),
    NeedInput,
    Eof,
}

/// Open, use, and drop on one execution lane, like the FFmpeg decoder it replaces.
/// Only the output callback runs on VideoToolbox's thread; it touches `shared` alone.
pub struct VideoToolboxDecoder {
    session: VTDecompressionSessionRef,
    format_description: CMFormatDescriptionRef,
    shared: Arc<Shared>,
    time_base: Rational,
    color_space: color::Space, // 流参数里的 YUV→RGB 矩阵标签；会话输出的图像不带 FFmpeg 帧属性。
    reorder_depth: usize,      // 展示顺序最多落后解码顺序的帧数（流的 video_delay）。
    draining: bool,            // 已送出 EOF；剩余帧全部完成后按展示顺序吐出。
    leading: LeadingPictures,
}

/// HEVC RASL pictures reference frames before their CRA. When decoding starts
/// at that CRA (stream start or after a flush), the spec drops them; FFmpeg's
/// decoder does the same. VideoToolbox would report missing references instead.
struct LeadingPictures {
    hevc: bool,
    nal_length_size: usize, // 每个 NAL 前的长度字段字节数（hvcC/avcC 中给出）。
    at_random_access: bool, // 下一个 IRAP 是解码起点：其 RASL 帧无参考可用。
    skip_rasl: bool,        // 当前 IRAP 是解码起点，跳过随后的 RASL 帧。
}

// Public APIs only
impl VideoToolboxDecoder {
    /// Errors for streams it cannot decode: other codecs, Annex B framing, or
    /// pixel layouts other than 4:2:0 at 8 or 10 bits.
    pub fn open(parameters: *const ffi::AVCodecParameters, time_base: Rational) -> Result<Self> {
        // SAFETY: the caller passes parameters owned by an open stream.
        let parameters = unsafe { &*parameters };
        let (codec_type, atom) = match parameters.codec_id {
            ffi::AVCodecID::AV_CODEC_ID_H264 => (u32::from_be_bytes(*b"avc1"), "avcC"),
            ffi::AVCodecID::AV_CODEC_ID_HEVC => (u32::from_be_bytes(*b"hvc1"), "hvcC"),
            other => bail!("unsupported video codec {other:?}: macOS decodes H.264 and HEVC"),
        };
        if parameters.extradata.is_null() || parameters.extradata_size <= 0 {
            bail!("video stream has no {atom} codec configuration");
        }
        // SAFETY: FFmpeg owns extradata_size bytes at extradata for the stream's lifetime.
        let extradata = unsafe {
            std::slice::from_raw_parts(parameters.extradata, parameters.extradata_size as usize)
        };
        if extradata[0] != 1 {
            // 不是 avcC/hvcC（版本号 1），而是 Annex B 起始码：包也不是长度前缀格式。
            bail!("video stream uses Annex B framing; only MP4-style {atom} streams are supported");
        }
        let pixel_format = output_pixel_format(parameters)?;
        // lengthSizeMinusOne: low two bits of byte 4 (avcC) or byte 21 (hvcC).
        let hevc = parameters.codec_id == ffi::AVCodecID::AV_CODEC_ID_HEVC;
        let length_offset = if hevc { 21 } else { 4 };
        let Some(length_byte) = extradata.get(length_offset) else {
            bail!("video {atom} configuration is truncated");
        };

        let format_description = create_format_description(
            codec_type,
            atom,
            parameters.width,
            parameters.height,
            extradata,
        )?;
        let shared = Arc::new(Shared {
            outputs: Mutex::new(Outputs {
                pictures: Vec::new(),
                in_flight: 0,
                error: None,
            }),
            changed: Condvar::new(),
        });
        let session = match create_session(format_description, pixel_format, &shared) {
            Ok(session) => session,
            Err(error) => {
                // SAFETY: created above and not shared.
                unsafe { CFRelease(format_description as CFTypeRef) };
                return Err(error);
            }
        };
        Ok(Self {
            session,
            format_description,
            shared,
            time_base,
            color_space: color::Space::from(parameters.color_space),
            reorder_depth: usize::try_from(parameters.video_delay).unwrap_or(0),
            draining: false,
            leading: LeadingPictures {
                hevc,
                nal_length_size: usize::from(length_byte & 0b11) + 1,
                at_random_access: true,
                skip_rasl: false,
            },
        })
    }

    /// Starts decoding without waiting. `output` false decodes the picture as a
    /// reference only; it never comes back from [`Self::receive`].
    pub fn send_packet(&mut self, packet: &Packet, output: bool) -> Result<()> {
        if self.draining {
            bail!("video decoder received a packet after draining began");
        }
        if self.leading.hevc {
            let Some(data) = packet.data() else {
                bail!("video packet has no data");
            };
            match first_picture_nal_type(data, self.leading.nal_length_size)? {
                // IRAP pictures: BLA, IDR, CRA, and reserved IRAP types.
                Some(16..=23) => {
                    self.leading.skip_rasl = self.leading.at_random_access;
                    self.leading.at_random_access = false;
                }
                Some(8 | 9) if self.leading.skip_rasl => return Ok(()), // RASL：不送入，也不会输出。
                Some(_) | None => {
                    // do nothing
                }
            }
        }
        let sample = sample_buffer(self.format_description, packet, self.time_base)?;
        let flags = match output {
            true => DECODE_ASYNC,
            false => DECODE_ASYNC | DECODE_DO_NOT_OUTPUT,
        };
        // Count before decoding: the callback may run before DecodeFrame returns.
        self.lock_outputs()?.in_flight += 1;
        let mut info = 0;
        // SAFETY: session and sample are valid; the sample is released after the call.
        let status = unsafe {
            VTDecompressionSessionDecodeFrame(self.session, sample, flags, null_mut(), &mut info)
        };
        unsafe { CFRelease(sample as CFTypeRef) };
        if status != 0 {
            bail!("VideoToolbox could not decode a frame: OSStatus {status}");
        }
        Ok(())
    }

    /// The next picture in presentation order, once no later-decoded frame can
    /// precede it. Blocks while enough frames are in flight to keep the decoder busy.
    pub fn receive(&mut self) -> Result<Received> {
        let mut outputs = self.lock_outputs()?;
        loop {
            if let Some(status) = outputs.error {
                bail!("VideoToolbox failed to decode a frame: OSStatus {status}");
            }
            let settled = self.draining && outputs.in_flight == 0;
            if outputs.pictures.len() > self.reorder_depth
                || (settled && !outputs.pictures.is_empty())
            {
                let earliest = outputs
                    .pictures
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, picture)| picture.pts)
                    .map(|(index, _)| index)
                    .expect("pictures is not empty");
                let picture = outputs.pictures.swap_remove(earliest);
                // SAFETY: the callback retained this buffer; ownership moves to the wrapper.
                let pixel_buffer =
                    unsafe { CVPixelBuffer::wrap_under_create_rule(picture.pixel_buffer.0) };
                return Ok(Received::Picture(DecodedPicture {
                    pixel_buffer,
                    pts: picture.pts,
                    duration: picture.duration,
                    color_space: self.color_space,
                }));
            }
            if settled {
                return Ok(Received::Eof);
            }
            if !self.draining && outputs.in_flight < MAX_IN_FLIGHT {
                return Ok(Received::NeedInput);
            }
            outputs = match self.shared.changed.wait(outputs) {
                Ok(outputs) => outputs,
                Err(_) => bail!("VideoToolbox output state was poisoned"),
            };
        }
    }

    /// No more packets: remaining pictures come out once every frame completes.
    pub fn send_eof(&mut self) {
        self.draining = true;
    }

    /// Discards pending pictures before a seek. The next packet must start a
    /// group of pictures.
    pub fn flush(&mut self) -> Result<()> {
        // SAFETY: session is valid for the decoder's lifetime.
        let status = unsafe { VTDecompressionSessionWaitForAsynchronousFrames(self.session) };
        if status != 0 {
            bail!("VideoToolbox could not finish pending frames: OSStatus {status}");
        }
        let mut outputs = self.lock_outputs()?;
        if let Some(status) = outputs.error {
            bail!("VideoToolbox failed to decode a frame: OSStatus {status}");
        }
        for picture in outputs.pictures.drain(..) {
            // SAFETY: each pending buffer was retained by the callback.
            unsafe { CFRelease(picture.pixel_buffer.0 as CFTypeRef) };
        }
        drop(outputs);
        self.draining = false;
        self.leading.at_random_access = true;
        Ok(())
    }
}

impl Drop for VideoToolboxDecoder {
    fn drop(&mut self) {
        // SAFETY: waiting first guarantees no callback runs after the session is gone.
        unsafe {
            VTDecompressionSessionWaitForAsynchronousFrames(self.session);
            VTDecompressionSessionInvalidate(self.session);
            CFRelease(self.session as CFTypeRef);
            CFRelease(self.format_description as CFTypeRef);
        }
        if let Ok(mut outputs) = self.shared.outputs.lock() {
            for picture in outputs.pictures.drain(..) {
                // SAFETY: each pending buffer was retained by the callback.
                unsafe { CFRelease(picture.pixel_buffer.0 as CFTypeRef) };
            }
        }
    }
}

// Private APIs only
impl VideoToolboxDecoder {
    fn lock_outputs(&self) -> Result<std::sync::MutexGuard<'_, Outputs>> {
        match self.shared.outputs.lock() {
            Ok(outputs) => Ok(outputs),
            Err(_) => bail!("VideoToolbox output state was poisoned"),
        }
    }
}

/// Most frames decoding at once. Sending the next frame before the previous
/// one finishes keeps the hardware decoder busy instead of waiting on each frame.
const MAX_IN_FLIGHT: usize = 4;

const DECODE_ASYNC: u32 = 1 << 0; // kVTDecodeFrame_EnableAsynchronousDecompression
const DECODE_DO_NOT_OUTPUT: u32 = 1 << 1; // kVTDecodeFrame_DoNotOutputFrame
const CM_TIME_VALID: u32 = 1; // kCMTimeFlags_Valid
const BLOCK_BUFFER_ASSURE_MEMORY_NOW: u32 = 1; // kCMBlockBufferAssureMemoryNowFlag

struct Shared {
    outputs: Mutex<Outputs>,
    changed: Condvar,
}

struct Outputs {
    pictures: Vec<PendingPicture>, // 已完成、尚未按展示顺序取走的帧（解码顺序）。
    in_flight: usize,              // 已送入、回调尚未返回的帧数。
    error: Option<i32>,            // 第一个失败的回调状态；之后的解码不再可信。
}

struct PendingPicture {
    pixel_buffer: RetainedPixelBuffer,
    pts: i64,
    duration: i64,
}

/// A retained CVPixelBuffer moving from VideoToolbox's callback thread.
struct RetainedPixelBuffer(CVPixelBufferRef);

// SAFETY: CoreVideo reference counting is thread-safe, and the buffer is not
// read until the owning lane takes it out of the queue.
unsafe impl Send for RetainedPixelBuffer {}

extern "C" fn decoded(
    refcon: *mut c_void,
    _frame_refcon: *mut c_void,
    status: i32,
    _info: u32,
    image: CVPixelBufferRef,
    pts: CMTime,
    duration: CMTime,
) {
    // SAFETY: refcon points at Shared, kept alive until the session is invalidated.
    let shared = unsafe { &*(refcon as *const Shared) };
    let Ok(mut outputs) = shared.outputs.lock() else {
        return; // 另一线程持锁时崩溃；解码线程会在下次加锁时报告。
    };
    outputs.in_flight = outputs.in_flight.saturating_sub(1);
    if status != 0 {
        if outputs.error.is_none() {
            outputs.error = Some(status);
        }
    } else if !image.is_null() {
        // SAFETY: VideoToolbox lends the buffer for the callback; retain to keep it.
        unsafe { CFRetain(image as CFTypeRef) };
        outputs.pictures.push(PendingPicture {
            pixel_buffer: RetainedPixelBuffer(image),
            pts: pts.value,
            duration: duration.value,
        });
    }
    // A null image without an error is a frame decoded only as a reference.
    drop(outputs);
    shared.changed.notify_all();
}

/// NAL unit type of the first picture slice in a length-prefixed HEVC packet;
/// None when the packet holds no slice (parameter sets or SEI only).
fn first_picture_nal_type(data: &[u8], length_size: usize) -> Result<Option<u8>> {
    let mut offset = 0;
    while offset < data.len() {
        let Some(length_bytes) = data.get(offset..offset + length_size) else {
            bail!("HEVC packet has a truncated NAL length");
        };
        let length = length_bytes
            .iter()
            .fold(0_usize, |length, byte| (length << 8) | usize::from(*byte));
        let start = offset + length_size;
        let Some(&header) = data.get(start) else {
            bail!("HEVC packet has a truncated NAL header");
        };
        let nal_type = (header >> 1) & 0x3f;
        if nal_type < 32 {
            return Ok(Some(nal_type)); // 0–31 为 VCL（图像切片）。
        }
        offset = start + length;
    }
    Ok(None)
}

/// NV12 or its 10-bit form, in the stream's range. Other layouts are errors.
fn output_pixel_format(parameters: &ffi::AVCodecParameters) -> Result<i32> {
    // SAFETY: format holds an AVPixelFormat for video parameters.
    let format: ffi::AVPixelFormat = unsafe { std::mem::transmute(parameters.format) };
    // SAFETY: returns a static descriptor or null.
    let descriptor = unsafe { ffi::av_pix_fmt_desc_get(format) };
    if descriptor.is_null() {
        bail!("unknown video pixel format {format:?}");
    }
    // SAFETY: non-null descriptors are static.
    let descriptor = unsafe { &*descriptor };
    if descriptor.log2_chroma_w != 1 || descriptor.log2_chroma_h != 1 {
        bail!("unsupported video pixel format {format:?}: only 4:2:0 is supported");
    }
    let full_range = parameters.color_range == ffi::AVColorRange::AVCOL_RANGE_JPEG;
    let fourcc = match (descriptor.comp[0].depth, full_range) {
        (8, false) => *b"420v",
        (8, true) => *b"420f",
        (10, false) => *b"x420",
        (10, true) => *b"xf20",
        (depth, _) => {
            bail!(
                "unsupported video pixel format {format:?}: {depth}-bit; only 8 and 10 are supported"
            )
        }
    };
    Ok(i32::from_be_bytes(fourcc))
}

fn create_format_description(
    codec_type: u32,
    atom: &str,
    width: i32,
    height: i32,
    extradata: &[u8],
) -> Result<CMFormatDescriptionRef> {
    let atoms = CFDictionary::<CFString, CFType>::from_CFType_pairs(&[(
        CFString::new(atom),
        CFData::from_buffer(extradata).as_CFType(),
    )]);
    // SAFETY: framework constants are valid for the process lifetime.
    let atoms_key = unsafe {
        CFString::wrap_under_get_rule(kCMFormatDescriptionExtension_SampleDescriptionExtensionAtoms)
    };
    let extensions =
        CFDictionary::<CFString, CFType>::from_CFType_pairs(&[(atoms_key, atoms.as_CFType())]);
    let mut description = null_mut();
    // SAFETY: all arguments are valid CoreFoundation objects or plain values.
    let status = unsafe {
        CMVideoFormatDescriptionCreate(
            null(),
            codec_type,
            width,
            height,
            extensions.as_concrete_TypeRef(),
            &mut description,
        )
    };
    if status != 0 {
        bail!("creating a VideoToolbox format description: OSStatus {status}");
    }
    Ok(description)
}

fn create_session(
    format_description: CMFormatDescriptionRef,
    pixel_format: i32,
    shared: &Arc<Shared>,
) -> Result<VTDecompressionSessionRef> {
    // SAFETY: framework constants are valid for the process lifetime.
    let (hardware_key, format_key, io_surface_key, metal_key) = unsafe {
        (
            CFString::wrap_under_get_rule(
                kVTVideoDecoderSpecification_RequireHardwareAcceleratedVideoDecoder,
            ),
            CFString::wrap_under_get_rule(kCVPixelBufferPixelFormatTypeKey),
            CFString::wrap_under_get_rule(kCVPixelBufferIOSurfacePropertiesKey),
            CFString::wrap_under_get_rule(kCVPixelBufferMetalCompatibilityKey),
        )
    };
    let specification = CFDictionary::<CFString, CFType>::from_CFType_pairs(&[(
        hardware_key,
        CFBoolean::true_value().as_CFType(),
    )]);
    let empty = CFDictionary::<CFString, CFType>::from_CFType_pairs(&[]);
    let attributes = CFDictionary::<CFString, CFType>::from_CFType_pairs(&[
        (format_key, CFNumber::from(pixel_format).as_CFType()),
        (io_surface_key, empty.as_CFType()), // IOSurface 支撑：GPU 转换和 GPUI 表面都需要。
        (metal_key, CFBoolean::true_value().as_CFType()),
    ]);
    let callback = VTDecompressionOutputCallbackRecord {
        callback: decoded,
        refcon: Arc::as_ptr(shared) as *mut c_void,
    };
    let mut session = null_mut();
    // SAFETY: the callback record is copied by VideoToolbox; refcon outlives the session.
    let status = unsafe {
        VTDecompressionSessionCreate(
            null(),
            format_description,
            specification.as_concrete_TypeRef(),
            attributes.as_concrete_TypeRef(),
            &callback,
            &mut session,
        )
    };
    if status != 0 {
        bail!("creating a VideoToolbox decoding session: OSStatus {status}");
    }
    Ok(session)
}

/// Wraps one length-prefixed packet; timestamps stay in stream ticks.
fn sample_buffer(
    format_description: CMFormatDescriptionRef,
    packet: &Packet,
    time_base: Rational,
) -> Result<CMSampleBufferRef> {
    let Some(data) = packet.data() else {
        bail!("video packet has no data");
    };
    let Some(pts) = packet.pts().or(packet.dts()) else {
        bail!("video packet has no timestamp");
    };
    let mut block = null_mut();
    // SAFETY: CoreMedia allocates and owns the block; data is copied into it below.
    let status = unsafe {
        CMBlockBufferCreateWithMemoryBlock(
            null(),
            null_mut(),
            data.len(),
            null(),
            null(),
            0,
            data.len(),
            BLOCK_BUFFER_ASSURE_MEMORY_NOW,
            &mut block,
        )
    };
    if status != 0 {
        bail!("allocating a video sample: OSStatus {status}");
    }
    // SAFETY: block has data.len() bytes of storage.
    let status =
        unsafe { CMBlockBufferReplaceDataBytes(data.as_ptr().cast(), block, 0, data.len()) };
    if status != 0 {
        unsafe { CFRelease(block as CFTypeRef) };
        bail!("copying a video sample: OSStatus {status}");
    }
    let timing = CMSampleTimingInfo {
        duration: cm_time(packet.duration(), time_base),
        presentation_time_stamp: cm_time(pts, time_base),
        decode_time_stamp: match packet.dts() {
            Some(dts) => cm_time(dts, time_base),
            None => CMTime::INVALID,
        },
    };
    let size = data.len();
    let mut sample = null_mut();
    // SAFETY: block, description, timing, and size are valid for one sample.
    let status = unsafe {
        CMSampleBufferCreateReady(
            null(),
            block,
            format_description,
            1,
            1,
            &timing,
            1,
            &size,
            &mut sample,
        )
    };
    unsafe { CFRelease(block as CFTypeRef) };
    if status != 0 {
        bail!("creating a video sample: OSStatus {status}");
    }
    Ok(sample)
}

/// Stream ticks passed through unchanged. VideoToolbox only orders and hands
/// back these values, so the callback reads the original tick count from `value`.
fn cm_time(ticks: i64, time_base: Rational) -> CMTime {
    CMTime {
        value: ticks,
        timescale: time_base.denominator(), // 秒数不精确无妨（分子不一定为 1）；只用于往返传递。
        flags: CM_TIME_VALID,
        epoch: 0,
    }
}

type CMFormatDescriptionRef = *mut c_void;
type CMBlockBufferRef = *mut c_void;
type CMSampleBufferRef = *mut c_void;
type VTDecompressionSessionRef = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct CMTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64,
}

impl CMTime {
    const INVALID: Self = Self {
        value: 0,
        timescale: 0,
        flags: 0,
        epoch: 0,
    };
}

#[repr(C)]
struct CMSampleTimingInfo {
    duration: CMTime,
    presentation_time_stamp: CMTime,
    decode_time_stamp: CMTime,
}

#[repr(C)]
struct VTDecompressionOutputCallbackRecord {
    callback: extern "C" fn(*mut c_void, *mut c_void, i32, u32, CVPixelBufferRef, CMTime, CMTime),
    refcon: *mut c_void,
}

#[link(name = "CoreMedia", kind = "framework")]
unsafe extern "C" {
    static kCMFormatDescriptionExtension_SampleDescriptionExtensionAtoms: CFStringRef;
    fn CMVideoFormatDescriptionCreate(
        allocator: *const c_void,
        codec_type: u32,
        width: i32,
        height: i32,
        extensions: CFDictionaryRef,
        out: *mut CMFormatDescriptionRef,
    ) -> i32;
    fn CMBlockBufferCreateWithMemoryBlock(
        allocator: *const c_void,
        memory_block: *mut c_void,
        block_length: usize,
        block_allocator: *const c_void,
        custom_block_source: *const c_void,
        offset_to_data: usize,
        data_length: usize,
        flags: u32,
        out: *mut CMBlockBufferRef,
    ) -> i32;
    fn CMBlockBufferReplaceDataBytes(
        source: *const c_void,
        destination: CMBlockBufferRef,
        offset: usize,
        length: usize,
    ) -> i32;
    fn CMSampleBufferCreateReady(
        allocator: *const c_void,
        data_buffer: CMBlockBufferRef,
        format_description: CMFormatDescriptionRef,
        num_samples: isize,
        num_timing_entries: isize,
        timing: *const CMSampleTimingInfo,
        num_size_entries: isize,
        sizes: *const usize,
        out: *mut CMSampleBufferRef,
    ) -> i32;
}

#[link(name = "VideoToolbox", kind = "framework")]
unsafe extern "C" {
    static kVTVideoDecoderSpecification_RequireHardwareAcceleratedVideoDecoder: CFStringRef;
    fn VTDecompressionSessionCreate(
        allocator: *const c_void,
        format_description: CMFormatDescriptionRef,
        decoder_specification: CFDictionaryRef,
        destination_attributes: CFDictionaryRef,
        output_callback: *const VTDecompressionOutputCallbackRecord,
        out: *mut VTDecompressionSessionRef,
    ) -> i32;
    fn VTDecompressionSessionDecodeFrame(
        session: VTDecompressionSessionRef,
        sample: CMSampleBufferRef,
        flags: u32,
        frame_refcon: *mut c_void,
        info_flags_out: *mut u32,
    ) -> i32;
    fn VTDecompressionSessionWaitForAsynchronousFrames(session: VTDecompressionSessionRef) -> i32;
    fn VTDecompressionSessionInvalidate(session: VTDecompressionSessionRef);
}

#[link(name = "CoreVideo", kind = "framework")]
unsafe extern "C" {
    static kCVPixelBufferPixelFormatTypeKey: CFStringRef;
    static kCVPixelBufferIOSurfacePropertiesKey: CFStringRef;
    static kCVPixelBufferMetalCompatibilityKey: CFStringRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRetain(object: CFTypeRef) -> CFTypeRef;
    fn CFRelease(object: CFTypeRef);
}
