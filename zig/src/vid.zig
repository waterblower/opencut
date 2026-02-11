const std = @import("std");
const print = std.debug.print;

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
    video_stream_idx: i32,
    width: i32,
    height: i32,
    fps: f64,
    finished: bool,

    pub fn frameDurationMs(self: *const Video) u64 {
        return @intFromFloat(1000.0 / self.fps);
    }

    pub fn isFinished(self: *const Video) bool {
        return self.finished;
    }

    pub fn restart(self: *Video) !void {
        _ = c.av_seek_frame(self.fmt_ctx, self.video_stream_idx, 0, c.AVSEEK_FLAG_BACKWARD);
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

        // 直接解码到 self.frame
        var ret = c.avcodec_receive_frame(self.codec_ctx, frame);

        while (ret == c.AVERROR(c.EAGAIN)) {
            while (true) {
                if (c.av_read_frame(self.fmt_ctx, self.packet) < 0) {
                    self.finished = true;
                    return null;
                }
                const stream_idx = self.packet.*.stream_index;
                if (stream_idx == self.video_stream_idx) {
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

    pub fn renderNextFrame(self: *Video, dest: [*]u8, dest_pitch: i32) !bool {
        const t0 = std.time.milliTimestamp();
        if (self.finished) {
            return false;
        }

        // 1. Allocate the frame locally.
        var frame = c.av_frame_alloc();
        if (frame == null) {
            return error.av_frame_alloc_failed;
        }
        // 2. Ensure it is freed when this function returns (CLEANUP).
        // &frame passes the address of your pointer, allowing FFmpeg to set it to null after freeing.
        defer c.av_frame_free(&frame);

        // Try to receive a decoded video frame
        var ret = c.avcodec_receive_frame(self.codec_ctx, frame);

        // If we need more data, read packets until we get a video frame
        while (ret == c.AVERROR(c.EAGAIN)) {
            while (true) {
                if (c.av_read_frame(self.fmt_ctx, self.packet) < 0) {
                    self.finished = true;
                    return false;
                }
                const stream_idx = self.packet.*.stream_index;
                if (stream_idx == self.video_stream_idx) {
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

        const t1 = std.time.milliTimestamp();

        // Setup destination arrays for sws_scale
        // sws_scale expects array of pointers and array of strides
        var dst_data: [4]?[*]u8 = undefined;
        var dst_linesize: [4]c_int = undefined;

        dst_data[0] = dest;
        dst_data[1] = null;
        dst_data[2] = null;
        dst_data[3] = null;

        dst_linesize[0] = dest_pitch;
        dst_linesize[1] = 0;
        dst_linesize[2] = 0;
        dst_linesize[3] = 0;

        // Convert to RGB directly into destination
        _ = c.sws_scale(
            self.sws_ctx,
            @ptrCast(&frame.*.data),
            @ptrCast(&frame.*.linesize),
            0,
            self.height,
            @ptrCast(&dst_data),
            @ptrCast(&dst_linesize),
        );
        const t2 = std.time.milliTimestamp();
        print("renderNextFrame total: {d}ms (decode: {d}ms, scale: {d}ms)\n", .{ t2 - t0, t1 - t0, t2 - t1 });
        return true;
    }
};

pub fn openVideo(allocator: std.mem.Allocator, file_path: []const u8) !*Video {
    // Ensure null-terminated string
    print("open {s}\n", .{file_path});
    const path_z = try allocator.dupeZ(u8, file_path);
    defer allocator.free(path_z);

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
    var video_stream_idx: i32 = -1;
    var i: c_uint = 0;
    while (i < fmt_ctx.?.nb_streams) : (i += 1) {
        if (fmt_ctx.?.streams[i].*.codecpar.*.codec_type == c.AVMEDIA_TYPE_VIDEO) {
            video_stream_idx = @intCast(i);
            break;
        }
    }

    if (video_stream_idx == -1) {
        return error.NoVideoStream;
    }

    // Get codec parameters
    const codecpar = fmt_ctx.?.streams[@intCast(video_stream_idx)].*.codecpar;

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
    const stream = fmt_ctx.?.streams[@intCast(video_stream_idx)];
    var fps: f64 = 30.0;
    if (stream.*.avg_frame_rate.den > 0) {
        const calc_fps = @as(f64, @floatFromInt(stream.*.avg_frame_rate.num)) / @as(f64, @floatFromInt(stream.*.avg_frame_rate.den));
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
    const video = try allocator.create(Video);
    video.* = Video{
        .allocator = allocator,
        .fmt_ctx = fmt_ctx.?,
        .codec_ctx = codec_ctx.?,
        .packet = packet.?,
        .sws_ctx = sws_ctx.?,
        .sws_frame = sws_frame.?,
        .video_stream_idx = video_stream_idx,
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

    print("Video opened:\n", .{});
    print("  Resolution:  {}x{} @ {d:.2} fps\n", .{ video_width, video_height, fps });
    print("  Codec:       {s}\n", .{codec_name});
    print("  Container:   {s}\n", .{format_name});
    print("  Color Space: {s}\n", .{color_space});

    return video;
}

pub fn split_half(
    allocator: std.mem.Allocator,
    input_path: []const u8,
    output1_path: []const u8,
    output2_path: []const u8,
) !void {
    const in_path_z = try allocator.dupeZ(u8, input_path);
    const out1_path_z = try allocator.dupeZ(u8, output1_path);
    const out2_path_z = try allocator.dupeZ(u8, output2_path);
    defer {
        allocator.free(in_path_z);
        allocator.free(out1_path_z);
        allocator.free(out2_path_z);
    }

    // --- 1. 打开输入文件 ---
    var in_ctx: ?*c.AVFormatContext = null;
    if (c.avformat_open_input(
        &in_ctx,
        in_path_z.ptr,
        null,
        null,
    ) < 0) {
        return error.OpenInputFailed;
    }
    defer c.avformat_close_input(@ptrCast(&in_ctx));

    if (c.avformat_find_stream_info(in_ctx, null) < 0) return error.FindStreamInfoFailed;

    // 获取总时长并计算中点
    const total_duration = in_ctx.?.*.duration;
    const half_duration = @divTrunc(total_duration, 2);
    const av_time_base_q = c.AVRational{
        .num = 1,
        .den = c.AV_TIME_BASE,
    };

    // --- 2. 准备输出文件 Context ---
    var out1_ctx: ?*c.AVFormatContext = null;
    var out2_ctx: ?*c.AVFormatContext = null;

    _ = c.avformat_alloc_output_context2(
        &out1_ctx,
        null,
        null,
        out1_path_z.ptr,
    );
    _ = c.avformat_alloc_output_context2(
        &out2_ctx,
        null,
        null,
        out2_path_z.ptr,
    );
    if (out1_ctx == null or out2_ctx == null) return error.OutputContextFailed;

    // 为了安全释放，使用 defer
    defer if (out1_ctx != null) c.avformat_free_context(out1_ctx);
    defer if (out2_ctx != null) c.avformat_free_context(out2_ctx);

    // --- 3. 映射并克隆所有流 (Video, Audio, Subs) ---
    const nb_streams = in_ctx.?.*.nb_streams;
    var i: c_uint = 0;
    while (i < nb_streams) : (i += 1) {
        const in_stream = in_ctx.?.*.streams[i];

        // Output 1 Stream
        const out1_stream = c.avformat_new_stream(out1_ctx, null);
        if (c.avcodec_parameters_copy(out1_stream.?.*.codecpar, in_stream.*.codecpar) < 0) return error.CopyParamsFailed;
        out1_stream.?.*.codecpar.*.codec_tag = 0; // 让 FFmpeg 自动分配 Tag

        // Output 2 Stream
        const out2_stream = c.avformat_new_stream(out2_ctx, null);
        if (c.avcodec_parameters_copy(out2_stream.?.*.codecpar, in_stream.*.codecpar) < 0) return error.CopyParamsFailed;
        out2_stream.?.*.codecpar.*.codec_tag = 0;
    }

    // --- 4. 打开物理文件并写入 Header ---
    if ((out1_ctx.?.*.oformat.*.flags & c.AVFMT_NOFILE) == 0) {
        if (c.avio_open(&out1_ctx.?.*.pb, out1_path_z.ptr, c.AVIO_FLAG_WRITE) < 0) {
            return error.IOOpenFailed;
        }
    }
    if ((out1_ctx.?.*.oformat.*.flags & c.AVFMT_NOFILE) == 0) {
        defer _ = c.avio_closep(&out1_ctx.?.*.pb);
    }

    if ((out2_ctx.?.*.oformat.*.flags & c.AVFMT_NOFILE) == 0) {
        if (c.avio_open(&out2_ctx.?.*.pb, out2_path_z.ptr, c.AVIO_FLAG_WRITE) < 0) return error.IOOpenFailed;
    }
    if ((out2_ctx.?.*.oformat.*.flags & c.AVFMT_NOFILE) == 0) {
        defer _ = c.avio_closep(&out2_ctx.?.*.pb);
    }

    if (c.avformat_write_header(out1_ctx, null) < 0) return error.WriteHeaderFailed;
    if (c.avformat_write_header(out2_ctx, null) < 0) return error.WriteHeaderFailed;

    // --- 5. 核心逻辑：逐包读取并分发 ---
    var packet = c.av_packet_alloc();
    if (packet == null) return error.PacketAllocFailed;
    defer c.av_packet_free(&packet);

    var is_second_half = false;

    // 用于记录第二段视频中，各个流的初始时间戳，以便将其归零
    const offset_pts = try allocator.alloc(i64, nb_streams);
    const offset_dts = try allocator.alloc(i64, nb_streams);
    const offset_set = try allocator.alloc(bool, nb_streams);
    defer {
        allocator.free(offset_pts);
        allocator.free(offset_dts);
        allocator.free(offset_set);
    }
    @memset(offset_set, false);

    print("Splitting video... Half duration is {d} microseconds\n", .{half_duration});

    while (c.av_read_frame(in_ctx, packet) >= 0) {
        const stream_idx = @as(usize, @intCast(packet.*.stream_index));
        const in_stream = in_ctx.?.*.streams[stream_idx];

        // 计算当前包在全局时间轴上的微秒数
        const ts_micros = c.av_rescale_q(packet.*.pts, in_stream.*.time_base, av_time_base_q);

        // 检查是否跨越了一半的时间线，并且当前包是**视频的关键帧**
        if (!is_second_half and ts_micros >= half_duration) {
            if (in_stream.*.codecpar.*.codec_type == c.AVMEDIA_TYPE_VIDEO) {
                if ((packet.*.flags & c.AV_PKT_FLAG_KEY) != 0) {
                    print("Split triggered at video keyframe! (ts: {d})\n", .{ts_micros});
                    is_second_half = true;
                }
            }
        }

        if (!is_second_half) {
            // 写入第一半
            const out_stream = out1_ctx.?.*.streams[stream_idx];
            c.av_packet_rescale_ts(packet, in_stream.*.time_base, out_stream.*.time_base);
            packet.*.pos = -1;
            _ = c.av_interleaved_write_frame(out1_ctx, packet);
        } else {
            // 写入第二半 (需要调整时间戳)
            const out_stream = out2_ctx.?.*.streams[stream_idx];

            // 记录偏移量
            if (!offset_set[stream_idx]) {
                offset_pts[stream_idx] = packet.*.pts;
                offset_dts[stream_idx] = packet.*.dts;
                offset_set[stream_idx] = true;
            }

            // 将 PTS/DTS 归零偏移
            packet.*.pts -= offset_pts[stream_idx];
            packet.*.dts -= offset_dts[stream_idx];

            // 转换到目标 TimeBase
            c.av_packet_rescale_ts(packet, in_stream.*.time_base, out_stream.*.time_base);
            packet.*.pos = -1;
            _ = c.av_interleaved_write_frame(out2_ctx, packet);
        }

        c.av_packet_unref(packet);
    }

    // --- 6. 写入尾部收尾 ---
    _ = c.av_write_trailer(out1_ctx);
    _ = c.av_write_trailer(out2_ctx);

    print("Split completed successfully!\n", .{});
}

pub fn split_half_v2(
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

    // Duration in AV_TIME_BASE units
    const duration = in_ctx.duration;
    if (duration <= 0)
        return error.NoDuration;

    const half_point = @divTrunc(duration, 2);

    var out1: ?*c.AVFormatContext = null;
    var out2: ?*c.AVFormatContext = null;

    // Create output contexts
    if (c.avformat_alloc_output_context2(&out1, null, null, output1_path) < 0)
        return error.Output1AllocFailed;

    if (c.avformat_alloc_output_context2(&out2, null, null, output2_path) < 0)
        return error.Output2AllocFailed;

    defer c.avformat_free_context(out1);
    defer c.avformat_free_context(out2);

    // Copy stream layout
    var i: usize = 0;
    while (i < in_ctx.nb_streams) : (i += 1) {
        const in_stream = in_ctx.streams[i];

        const s1 = c.avformat_new_stream(out1, null) orelse return error.StreamCreate;
        const s2 = c.avformat_new_stream(out2, null) orelse return error.StreamCreate;

        if (c.avcodec_parameters_copy(s1.*.codecpar, in_stream.*.codecpar) < 0)
            return error.ParamCopy;

        if (c.avcodec_parameters_copy(s2.*.codecpar, in_stream.*.codecpar) < 0)
            return error.ParamCopy;

        s1.*.codecpar.*.codec_tag = 0;
        s2.*.codecpar.*.codec_tag = 0;
    }

    if (c.avio_open(&out1.?.pb, output1_path, c.AVIO_FLAG_WRITE) < 0)
        return error.OpenOutput1;

    if (c.avio_open(&out2.?.pb, output2_path, c.AVIO_FLAG_WRITE) < 0)
        return error.OpenOutput2;

    if (c.avformat_write_header(out1, null) < 0)
        return error.WriteHeader1;

    if (c.avformat_write_header(out2, null) < 0)
        return error.WriteHeader2;

    var pkt: c.AVPacket = undefined;
    c.av_init_packet(&pkt);

    defer c.av_packet_unref(&pkt);

    while (c.av_read_frame(in_ctx, &pkt) >= 0) {
        const stream = in_ctx.streams[@intCast(pkt.stream_index)];

        // Convert packet timestamp to AV_TIME_BASE
        const time = c.av_rescale_q(
            pkt.pts,
            stream.*.time_base,
            c.AV_TIME_BASE_Q,
        );

        const target_ctx =
            if (time < half_point) out1.? else out2.?;

        const out_stream = target_ctx.streams[@intCast(pkt.stream_index)];

        // Rescale timestamps
        pkt.pts = c.av_rescale_q(pkt.pts, stream.*.time_base, out_stream.*.time_base);
        pkt.dts = c.av_rescale_q(pkt.dts, stream.*.time_base, out_stream.*.time_base);
        pkt.duration = c.av_rescale_q(pkt.duration, stream.*.time_base, out_stream.*.time_base);

        pkt.pos = -1;

        if (c.av_interleaved_write_frame(target_ctx, &pkt) < 0) {
            return error.WritePacketFailed;
        }

        c.av_packet_unref(&pkt);
    }

    _ = c.av_write_trailer(out1);
    _ = c.av_write_trailer(out2);

    _ = c.avio_closep(&out1.?.pb);
    _ = c.avio_closep(&out2.?.pb);
}
