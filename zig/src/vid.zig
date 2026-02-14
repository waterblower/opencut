const std = @import("std");
const debug = std.debug.print;

const c = @cImport({
    @cInclude("libavformat/avformat.h");
    @cInclude("libavcodec/avcodec.h");
    @cInclude("libavutil/avutil.h");
    @cInclude("libavutil/imgutils.h");
    @cInclude("libswscale/swscale.h");
});

pub const Video = struct {
    allocator: std.mem.Allocator,
    fmt_ctx: *c.AVFormatContext,
    codec_ctx: *c.AVCodecContext,
    packet: *c.AVPacket,
    sws_ctx: ?*c.struct_SwsContext,
    sws_frame: *c.AVFrame,
    video_stream: c.AVStream,
    width: i32,
    height: i32,
    fps: f64,
    finished: bool,

    pub fn frameDurationMs(self: *const Video) u64 {
        return @intFromFloat(1000.0 / self.fps);
    }

    pub fn restart(self: *Video) !void {
        _ = c.av_seek_frame(self.fmt_ctx, self.video_stream.index, 0, c.AVSEEK_FLAG_BACKWARD);
        c.avcodec_flush_buffers(self.codec_ctx);
        self.finished = false;
    }

    pub fn deinit(self: *Video) void {
        var packet_ptr: ?*c.AVPacket = self.packet;
        c.av_packet_free(@ptrCast(&packet_ptr));
        c.sws_freeContext(self.sws_ctx);
        var codec_ctx_ptr: ?*c.AVCodecContext = self.codec_ctx;
        c.avcodec_free_context(@ptrCast(&codec_ctx_ptr));
        var fmt_ctx_ptr: ?*c.AVFormatContext = self.fmt_ctx;
        c.avformat_close_input(@ptrCast(&fmt_ctx_ptr));
        self.allocator.destroy(self);
    }

    // --- 核心变化: 相对于 renderNextFrame ---
    // 不再接受 dest/pitch，不再做 sws_scale
    // 它的唯一职责就是把 self.frame 填满数据
    pub fn read_next_frame(self: *Video) !?*c.AVFrame {
        if (self.finished) return null;

        const frame = c.av_frame_alloc();
        if (frame == null) {
            return error.av_frame_alloc_failed;
        }

        // 解码到 frame
        var ret = c.avcodec_receive_frame(self.codec_ctx, frame);

        while (ret == c.AVERROR(c.EAGAIN)) {
            while (true) {
                // 这个函数名是 FFmpeg 历史遗留的超级大坑
                // 它的名字叫“读帧”，但它实际上读出来的是 Packet（压缩包）！
                // 它从硬盘把数据搬运到内存
                if (c.av_read_frame(self.fmt_ctx, self.packet) < 0) {
                    self.finished = true;
                    return null;
                }
                const stream_idx = self.packet.*.stream_index;
                if (stream_idx == self.video_stream.index) {
                    _ = c.avcodec_send_packet(self.codec_ctx, self.packet);
                    c.av_packet_unref(self.packet);
                    break;
                }
                c.av_packet_unref(self.packet);
            }
            ret = c.avcodec_receive_frame(self.codec_ctx, frame);
        }

        if (ret < 0) {
            return error.FrameDecodeError;
        }

        // 成功！此时 frame 里面是最新的 YUV 数据
        // 外部可以通过 frame.data[0] 等访问
        return frame;
    }
};

pub fn open(a: std.mem.Allocator, file_path: []const u8) !*Video {
    // Ensure null-terminated string
    debug("open {s}\n", .{file_path});
    const path_z = try a.dupeZ(u8, file_path);
    defer a.free(path_z);

    var fmt_ctx: ?*c.AVFormatContext = null;

    // Open video file
    if (c.avformat_open_input(&fmt_ctx, path_z.ptr, null, null) < 0) {
        return error.CouldNotOpenFile;
    }
    errdefer c.avformat_close_input(@ptrCast(&fmt_ctx));

    // Retrieve stream information
    if (c.avformat_find_stream_info(fmt_ctx, null) < 0) {
        return error.CouldNotFindStreamInfo;
    }

    // Find the first video stream
    const video_stream = try find_1st_video_stream(fmt_ctx.?);

    // Get codec parameters
    const codecpar = video_stream.codecpar;

    // Find decoder
    const codec = c.avcodec_find_decoder(codecpar.*.codec_id);
    if (codec == null) {
        return error.UnsupportedCodec;
    }

    // Allocate codec context
    const codec_ctx = c.avcodec_alloc_context3(codec);
    if (codec_ctx == null) {
        return error.CouldNotAllocateCodecContext;
    }
    errdefer {
        var codec_ctx_ptr = codec_ctx;
        c.avcodec_free_context(@ptrCast(&codec_ctx_ptr));
    }

    // Copy codec parameters to context
    if (c.avcodec_parameters_to_context(codec_ctx, codecpar) < 0) {
        return error.CouldNotCopyCodecParams;
    }

    // Enable multithreading
    codec_ctx.?.*.thread_count = 0;

    // Open codec
    if (c.avcodec_open2(codec_ctx, codec, null) < 0) {
        return error.CouldNotOpenCodec;
    }

    const video_width = codec_ctx.?.*.width;
    const video_height = codec_ctx.?.*.height;

    // Calculate frame rate (default to 30fps if we can't determine)
    var fps: f64 = 30.0;
    if (video_stream.avg_frame_rate.den > 0) {
        const calc_fps = @as(f64, @floatFromInt(video_stream.avg_frame_rate.num)) / @as(
            f64,
            @floatFromInt(video_stream.avg_frame_rate.den),
        );
        if (calc_fps > 0) {
            fps = calc_fps;
        }
    }

    // Allocate packet
    const packet = c.av_packet_alloc();
    if (packet == null) {
        return error.CouldNotAllocatePacket;
    }
    errdefer {
        var packet_ptr = packet;
        c.av_packet_free(@ptrCast(&packet_ptr));
    }

    // Initialize SWS context for color conversion
    var sws_ctx: ?*c.SwsContext = null;
    if (codec_ctx.?.*.pix_fmt == c.AV_PIX_FMT_YUV420P10LE) {
        sws_ctx = c.sws_getContext(
            video_width,
            video_height,
            c.AV_PIX_FMT_YUV420P10LE, // 源格式
            video_width,
            video_height,
            c.AV_PIX_FMT_YUV420P, // 目标格式（8bit）
            c.SWS_BILINEAR,
            null,
            null,
            null,
        );
        if (sws_ctx == null) {
            return error.CouldNotInitSWSContext;
        }
        errdefer c.sws_freeContext(sws_ctx);
    }

    // 准备 sws_frame
    var sws_frame: ?*c.AVFrame = c.av_frame_alloc();
    if (sws_frame == null) {
        return error.FrameAllocFailed;
    }

    sws_frame.?.format = c.AV_PIX_FMT_YUV420P;
    sws_frame.?.width = video_width;
    sws_frame.?.height = video_height;

    const ret = c.av_frame_get_buffer(sws_frame, 32);
    if (ret < 0) {
        return error.FrameBufferAllocFailed;
    }

    // Allocate buffer for RGB frame
    const num_bytes = c.av_image_get_buffer_size(c.AV_PIX_FMT_BGRA, video_width, video_height, 1);
    const buffer = c.av_malloc(@intCast(num_bytes));
    if (buffer == null) {
        return error.CouldNotAllocateBuffer;
    }

    // Create and return Video instance
    const video = try a.create(Video);
    video.* = Video{
        .allocator = a,
        .fmt_ctx = fmt_ctx.?,
        .codec_ctx = codec_ctx.?,
        .packet = packet.?,
        .sws_ctx = sws_ctx.?,
        .sws_frame = sws_frame.?,
        .video_stream = video_stream,
        .width = video_width,
        .height = video_height,
        .fps = fps,
        .finished = false,
    };

    // Get codec name
    const codec_name = if (codec.?.*.long_name != null)
        std.mem.span(codec.?.*.long_name)
    else if (codec.?.*.name != null)
        std.mem.span(codec.?.*.name)
    else
        "unknown";

    // Get container format name
    const format_name = if (fmt_ctx.?.*.iformat.*.long_name != null)
        std.mem.span(fmt_ctx.?.*.iformat.*.long_name)
    else if (fmt_ctx.?.*.iformat.*.name != null)
        std.mem.span(fmt_ctx.?.*.iformat.*.name)
    else
        "unknown";

    // Get color space name
    const pix_fmt = codec_ctx.?.*.pix_fmt;
    const pix_fmt_name = c.av_get_pix_fmt_name(pix_fmt);
    const color_space = if (pix_fmt_name != null)
        std.mem.span(pix_fmt_name)
    else
        "unknown";

    debug("Video opened:\n", .{});
    debug("  Resolution:  {}x{} @ {d:.2} fps\n", .{ video_width, video_height, fps });
    debug("  Codec:       {s}\n", .{codec_name});
    debug("  Container:   {s}\n", .{format_name});
    debug("  Color Space: {s}\n", .{color_space});

    return video;
}

pub fn split_half(
    input_path: [*:0]const u8,
    output1_path: [*:0]const u8,
    output2_path: [*:0]const u8,
) !void {
    var ifmt_ctx: ?*c.AVFormatContext = null;
    if (c.avformat_open_input(&ifmt_ctx, input_path, null, null) < 0)
        return error.OpenInputFailed;
    defer c.avformat_close_input(&ifmt_ctx);

    if (c.avformat_find_stream_info(ifmt_ctx, null) < 0)
        return error.StreamInfoFailed;

    const in_ctx = ifmt_ctx.?;
    const duration = in_ctx.duration;
    if (duration <= 0) return error.NoDuration;

    const half_point = @divTrunc(duration, 2);
    var out1: ?*c.AVFormatContext = null;
    var out2: ?*c.AVFormatContext = null;

    if (c.avformat_alloc_output_context2(&out1, null, null, output1_path) < 0)
        return error.Output1AllocFailed;
    if (c.avformat_alloc_output_context2(&out2, null, null, output2_path) < 0)
        return error.Output2AllocFailed;

    defer c.avformat_free_context(out1);
    defer c.avformat_free_context(out2);

    // Track offsets for each stream to reset the second half to 0
    var stream_offsets = try std.heap.page_allocator.alloc(?i64, in_ctx.nb_streams);
    @memset(stream_offsets, null);
    defer std.heap.page_allocator.free(stream_offsets);

    var i: usize = 0;
    while (i < in_ctx.nb_streams) : (i += 1) {
        const in_stream = in_ctx.streams[i];
        const s1 = c.avformat_new_stream(out1, null) orelse return error.StreamCreate;
        const s2 = c.avformat_new_stream(out2, null) orelse return error.StreamCreate;
        if (c.avcodec_parameters_copy(s1.*.codecpar, in_stream.*.codecpar) < 0) return error.ParamCopy;
        if (c.avcodec_parameters_copy(s2.*.codecpar, in_stream.*.codecpar) < 0) return error.ParamCopy;
        s1.*.codecpar.*.codec_tag = 0;
        s2.*.codecpar.*.codec_tag = 0;
    }

    if (c.avio_open(&out1.?.pb, output1_path, c.AVIO_FLAG_WRITE) < 0) return error.OpenOutput1;
    if (c.avio_open(&out2.?.pb, output2_path, c.AVIO_FLAG_WRITE) < 0) return error.OpenOutput2;
    if (c.avformat_write_header(out1, null) < 0) return error.WriteHeader1;
    if (c.avformat_write_header(out2, null) < 0) return error.WriteHeader2;

    var pkt: c.AVPacket = undefined;
    c.av_init_packet(&pkt);

    while (c.av_read_frame(in_ctx, &pkt) >= 0) {
        const stream_idx = @as(usize, @intCast(pkt.stream_index));
        const in_stream = in_ctx.streams[stream_idx];

        // Convert current packet time to global microseconds for splitting logic
        const time = c.av_rescale_q(pkt.pts, in_stream.*.time_base, c.AV_TIME_BASE_Q);

        if (time < half_point) {
            // --- First Half ---
            const out_stream = out1.?.streams[stream_idx];
            pkt.pts = c.av_rescale_q(pkt.pts, in_stream.*.time_base, out_stream.*.time_base);
            pkt.dts = c.av_rescale_q(pkt.dts, in_stream.*.time_base, out_stream.*.time_base);
            pkt.duration = c.av_rescale_q(pkt.duration, in_stream.*.time_base, out_stream.*.time_base);
            pkt.pos = -1;
            if (c.av_interleaved_write_frame(out1, &pkt) < 0) return error.WritePacketFailed;
        } else {
            // --- Second Half ---
            const out_stream = out2.?.streams[stream_idx];

            // If this is the first packet for this stream in the second half, record the offset
            if (stream_offsets[stream_idx] == null) {
                stream_offsets[stream_idx] = pkt.pts;
            }

            // Subtract offset to normalize start to 0
            pkt.pts -= stream_offsets[stream_idx].?;
            pkt.dts -= stream_offsets[stream_idx].?;

            // Rescale to output stream timebase
            pkt.pts = c.av_rescale_q(pkt.pts, in_stream.*.time_base, out_stream.*.time_base);
            pkt.dts = c.av_rescale_q(pkt.dts, in_stream.*.time_base, out_stream.*.time_base);
            pkt.duration = c.av_rescale_q(pkt.duration, in_stream.*.time_base, out_stream.*.time_base);
            pkt.pos = -1;
            if (c.av_interleaved_write_frame(out2, &pkt) < 0) return error.WritePacketFailed;
        }
        c.av_packet_unref(&pkt);
    }

    _ = c.av_write_trailer(out1);
    _ = c.av_write_trailer(out2);
    _ = c.avio_closep(&out1.?.pb);
    _ = c.avio_closep(&out2.?.pb);
}

pub fn get_bit_depth(frame: *c.AVFrame) !i32 {
    // 1. 获取描述符
    const desc = c.av_pix_fmt_desc_get(@intCast(frame.format));

    if (desc == null) return error.Format_DESC_NULL;

    // 2. 读取第一个分量 (Component 0, 即 Y/Luma) 的深度
    // desc.comp 是一个数组，存储了 RGBA 或 YUVA 各个分量的信息
    return desc.*.comp[0].depth;
}

/// 查找所有视频流的索引
pub fn find_1st_video_stream(fmt_ctx: *c.AVFormatContext) !c.AVStream {
    var s: ?*c.AVStream = null;
    var i: c_uint = 0;
    // 遍历所有流
    while (i < fmt_ctx.nb_streams) : (i += 1) {
        const stream = fmt_ctx.streams[i];
        const codec_par = stream.*.codecpar;

        // 核心判断：是不是视频类型？
        if (s == null and codec_par.*.codec_type == c.AVMEDIA_TYPE_VIDEO) {

            // 【进阶技巧】：过滤掉封面图 (Cover Art)
            // 很多 MP3/MP4 会把封面图当成一个单帧的 MJPEG 视频流
            // 如果你只想要“能播的视频”，可以加上这个判断：
            const is_attached_pic = (stream.*.disposition & c.AV_DISPOSITION_ATTACHED_PIC) != 0;
            if (!is_attached_pic) {
                s = stream;
            }
        }
        debug("Stream #{d}: {d}\n", .{ i, stream.*.codecpar.*.codec_type });
    }
    if (s == null) {
        return error.NoVideoStreamFound;
    } else {
        return s.?.*;
    }
}
