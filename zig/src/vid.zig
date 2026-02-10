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
    rgb_frame: *c.AVFrame,
    packet: *c.AVPacket,
    sws_ctx: *c.struct_SwsContext,
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
        if (self.rgb_frame.*.data[0] != null) {
            c.av_free(self.rgb_frame.*.data[0]);
        }
        var rgb_frame_ptr: ?*c.AVFrame = self.rgb_frame;
        c.av_frame_free(@ptrCast(&rgb_frame_ptr));
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

    // Allocate RGB frame
    const rgb_frame = c.av_frame_alloc();
    if (rgb_frame == null) {
        return error.CouldNotAllocateRGBFrame;
    }
    errdefer {
        var rgb_frame_ptr = rgb_frame;
        c.av_frame_free(@ptrCast(&rgb_frame_ptr));
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
    const sws_ctx = c.sws_getContext(
        video_width,
        video_height,
        codec_ctx.?.*.pix_fmt,
        video_width,
        video_height,
        c.AV_PIX_FMT_BGRA,
        c.SWS_POINT,
        null,
        null,
        null,
    );
    if (sws_ctx == null) {
        return error.CouldNotInitSWSContext;
    }
    errdefer c.sws_freeContext(sws_ctx);

    // Allocate buffer for RGB frame
    const num_bytes = c.av_image_get_buffer_size(c.AV_PIX_FMT_BGRA, video_width, video_height, 1);
    const buffer = c.av_malloc(@intCast(num_bytes));
    if (buffer == null) {
        return error.CouldNotAllocateBuffer;
    }

    _ = c.av_image_fill_arrays(
        @ptrCast(&rgb_frame.?.*.data),
        @ptrCast(&rgb_frame.?.*.linesize),
        @ptrCast(buffer),
        c.AV_PIX_FMT_BGRA,
        video_width,
        video_height,
        1,
    );

    // Create and return Video instance
    const video = try allocator.create(Video);
    video.* = Video{
        .allocator = allocator,
        .fmt_ctx = fmt_ctx.?,
        .codec_ctx = codec_ctx.?,
        .rgb_frame = rgb_frame.?,
        .packet = packet.?,
        .sws_ctx = sws_ctx.?,
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
