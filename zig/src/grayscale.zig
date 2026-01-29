const std = @import("std");

const c = @cImport({
    @cInclude("libavformat/avformat.h");
    @cInclude("libavcodec/avcodec.h");
    @cInclude("libavutil/avutil.h");
    @cInclude("libavutil/imgutils.h");
    @cInclude("libswscale/swscale.h");
});

pub fn convertToGrayscale(allocator: std.mem.Allocator, input_path: []const u8, output_path: []const u8) !void {
    // Convert to C strings
    const input_cstr = try allocator.dupeZ(u8, input_path);
    defer allocator.free(input_cstr);
    const output_cstr = try allocator.dupeZ(u8, output_path);
    defer allocator.free(output_cstr);

    // Suppress FFmpeg logging
    c.av_log_set_level(c.AV_LOG_ERROR);

    // Open input file
    var input_ctx: ?*c.AVFormatContext = null;
    if (c.avformat_open_input(&input_ctx, input_cstr.ptr, null, null) < 0) {
        std.debug.print("Error: Could not open input file '{s}'\n", .{input_path});
        return error.CannotOpenInput;
    }
    defer c.avformat_close_input(&input_ctx);

    // Retrieve stream information
    if (c.avformat_find_stream_info(input_ctx, null) < 0) {
        std.debug.print("Error: Could not find stream information\n", .{});
        return error.StreamInfoNotFound;
    }

    // Find video stream
    var video_stream_idx: c_int = -1;
    var audio_stream_idx: c_int = -1;
    const nb_streams = input_ctx.?.nb_streams;

    var i: u32 = 0;
    while (i < nb_streams) : (i += 1) {
        const stream = input_ctx.?.streams[i];
        if (stream.*.codecpar.*.codec_type == c.AVMEDIA_TYPE_VIDEO and video_stream_idx == -1) {
            video_stream_idx = @intCast(i);
        } else if (stream.*.codecpar.*.codec_type == c.AVMEDIA_TYPE_AUDIO and audio_stream_idx == -1) {
            audio_stream_idx = @intCast(i);
        }
    }

    if (video_stream_idx == -1) {
        std.debug.print("Error: Could not find video stream\n", .{});
        return error.NoVideoStream;
    }

    const video_stream = input_ctx.?.streams[@intCast(video_stream_idx)];
    const video_codecpar = video_stream.*.codecpar;

    std.debug.print("Input video: {d}x{d}, codec: {s}, fps: {d}/{d}, bitrate: {d}\n", .{
        video_codecpar.*.width,
        video_codecpar.*.height,
        c.avcodec_get_name(video_codecpar.*.codec_id),
        video_stream.*.r_frame_rate.num,
        video_stream.*.r_frame_rate.den,
        video_codecpar.*.bit_rate,
    });

    // Find decoder
    const decoder = c.avcodec_find_decoder(video_codecpar.*.codec_id);
    if (decoder == null) {
        std.debug.print("Error: Decoder not found\n", .{});
        return error.DecoderNotFound;
    }

    // Allocate decoder context
    const decoder_ctx = c.avcodec_alloc_context3(decoder);
    if (decoder_ctx == null) {
        std.debug.print("Error: Could not allocate decoder context\n", .{});
        return error.DecoderAllocFailed;
    }
    defer c.avcodec_free_context(@constCast(&decoder_ctx));

    // Copy codec parameters to decoder context
    if (c.avcodec_parameters_to_context(decoder_ctx, video_codecpar) < 0) {
        std.debug.print("Error: Could not copy codec parameters\n", .{});
        return error.CodecParamsCopyFailed;
    }

    // Open decoder with low delay options
    var decoder_opts: ?*c.AVDictionary = null;
    defer c.av_dict_free(&decoder_opts);
    _ = c.av_dict_set(&decoder_opts, "threads", "1", 0); // Decoder threads
    _ = c.av_dict_set(&decoder_opts, "flags", "low_delay", 0);

    if (c.avcodec_open2(decoder_ctx, decoder, &decoder_opts) < 0) {
        std.debug.print("Error: Could not open decoder\n", .{});
        return error.DecoderOpenFailed;
    }

    // Set decoder to output all frames
    decoder_ctx.*.flags |= c.AV_CODEC_FLAG_OUTPUT_CORRUPT;
    decoder_ctx.*.flags2 |= c.AV_CODEC_FLAG2_SHOW_ALL;

    // Create output context
    var output_ctx: ?*c.AVFormatContext = null;
    if (c.avformat_alloc_output_context2(&output_ctx, null, null, output_cstr.ptr) < 0) {
        std.debug.print("Error: Could not create output context\n", .{});
        return error.OutputContextFailed;
    }
    defer {
        if (output_ctx) |ctx| {
            if ((ctx.oformat.*.flags & c.AVFMT_NOFILE) == 0) {
                _ = c.avio_closep(&ctx.pb);
            }
            c.avformat_free_context(ctx);
        }
    }

    // Create output video stream
    const out_video_stream = c.avformat_new_stream(output_ctx, null);
    if (out_video_stream == null) {
        std.debug.print("Error: Could not create output video stream\n", .{});
        return error.OutputStreamFailed;
    }

    // Force HEVC Encoder
    const encoder = c.avcodec_find_encoder(c.AV_CODEC_ID_HEVC);
    if (encoder == null) {
        std.debug.print("Error: HEVC Encoder not found (libx265 installed?)\n", .{});
        return error.EncoderNotFound;
    }

    std.debug.print("Using encoder: {s}\n", .{encoder.*.name});

    // Allocate encoder context
    const encoder_ctx = c.avcodec_alloc_context3(encoder);
    if (encoder_ctx == null) {
        std.debug.print("Error: Could not allocate encoder context\n", .{});
        return error.EncoderAllocFailed;
    }
    defer c.avcodec_free_context(@constCast(&encoder_ctx));

    // Copy encoder parameters from decoder
    encoder_ctx.*.width = decoder_ctx.*.width;
    encoder_ctx.*.height = decoder_ctx.*.height;
    encoder_ctx.*.sample_aspect_ratio = decoder_ctx.*.sample_aspect_ratio;
    encoder_ctx.*.pix_fmt = c.AV_PIX_FMT_YUV420P;

    // Set color space for QuickTime compatibility
    encoder_ctx.*.color_range = c.AVCOL_RANGE_MPEG;
    encoder_ctx.*.color_primaries = c.AVCOL_PRI_BT709;
    encoder_ctx.*.color_trc = c.AVCOL_TRC_BT709;
    encoder_ctx.*.colorspace = c.AVCOL_SPC_BT709;

    encoder_ctx.*.time_base = video_stream.*.time_base;
    encoder_ctx.*.framerate = video_stream.*.avg_frame_rate;
    if (video_stream.*.r_frame_rate.num > 0 and video_stream.*.r_frame_rate.den > 0) {
        encoder_ctx.*.framerate = video_stream.*.r_frame_rate;
    }

    // === FIX: Copy Bitrate strictly from input with proper casting ===
    if (video_codecpar.*.bit_rate > 0) {
        encoder_ctx.*.bit_rate = video_codecpar.*.bit_rate;
        encoder_ctx.*.rc_max_rate = video_codecpar.*.bit_rate;
        // Correctly cast i64 to c_int for buffer size
        encoder_ctx.*.rc_buffer_size = @intCast(video_codecpar.*.bit_rate * 2);
    } else {
        std.debug.print("Warning: Input bitrate unknown. Encoder will use defaults.\n", .{});
    }

    // Copy codec-specific parameters
    encoder_ctx.*.gop_size = decoder_ctx.*.gop_size;
    encoder_ctx.*.max_b_frames = decoder_ctx.*.max_b_frames;

    // Force compatible profile for HEVC
    encoder_ctx.*.profile = c.FF_PROFILE_HEVC_MAIN;

    if (output_ctx.?.oformat.*.flags & c.AVFMT_GLOBALHEADER != 0) {
        encoder_ctx.*.flags |= c.AV_CODEC_FLAG_GLOBAL_HEADER;
    }

    var encoder_opts: ?*c.AVDictionary = null;
    defer c.av_dict_free(&encoder_opts);

    // Force 1 Thread & Configure HEVC
    _ = c.av_dict_set(&encoder_opts, "threads", "1", 0);
    _ = c.av_dict_set(&encoder_opts, "preset", "ultrafast", 0);
    _ = c.av_dict_set(&encoder_opts, "x265-params", "log-level=error", 0);
    _ = c.av_dict_set(&encoder_opts, "tag", "hvc1", 0);

    if (encoder_ctx.*.bit_rate == 0) {
        _ = c.av_dict_set(&encoder_opts, "crf", "28", 0);
    }

    if (c.avcodec_open2(encoder_ctx, encoder, &encoder_opts) < 0) {
        std.debug.print("Error: Could not open encoder\n", .{});
        return error.EncoderOpenFailed;
    }

    // Copy encoder parameters to output stream
    if (c.avcodec_parameters_from_context(out_video_stream.*.codecpar, encoder_ctx) < 0) {
        std.debug.print("Error: Could not copy encoder parameters\n", .{});
        return error.EncoderParamsCopyFailed;
    }

    out_video_stream.*.time_base = encoder_ctx.*.time_base;
    out_video_stream.*.codecpar.*.codec_tag = c.MKTAG('h', 'v', 'c', '1');

    // Handle audio stream (copy without re-encoding)
    var out_audio_stream: ?*c.AVStream = null;
    var audio_stream_ptr: ?*c.AVStream = null;

    if (audio_stream_idx != -1) {
        audio_stream_ptr = input_ctx.?.streams[@intCast(audio_stream_idx)];
        out_audio_stream = c.avformat_new_stream(output_ctx, null);

        if (out_audio_stream != null) {
            _ = c.avcodec_parameters_copy(out_audio_stream.?.*.codecpar, audio_stream_ptr.?.*.codecpar);
            out_audio_stream.?.*.codecpar.*.codec_tag = 0;
            out_audio_stream.?.*.time_base = audio_stream_ptr.?.*.time_base;
        }
    }

    // Open output file
    if ((output_ctx.?.oformat.*.flags & c.AVFMT_NOFILE) == 0) {
        if (c.avio_open(&output_ctx.?.pb, output_cstr.ptr, c.AVIO_FLAG_WRITE) < 0) {
            std.debug.print("Error: Could not open output file\n", .{});
            return error.OutputFileOpenFailed;
        }
    }

    // Write output file header
    var muxer_opts: ?*c.AVDictionary = null;
    defer c.av_dict_free(&muxer_opts);
    _ = c.av_dict_set(&muxer_opts, "movflags", "+faststart", 0);

    if (c.avformat_write_header(output_ctx, &muxer_opts) < 0) {
        std.debug.print("Error: Could not write output header\n", .{});
        return error.WriteHeaderFailed;
    }

    // Create SwScale context
    const sws_ctx = c.sws_getContext(decoder_ctx.*.width, decoder_ctx.*.height, decoder_ctx.*.pix_fmt, encoder_ctx.*.width, encoder_ctx.*.height, encoder_ctx.*.pix_fmt, c.SWS_BILINEAR, null, null, null);
    if (sws_ctx == null) {
        std.debug.print("Error: Could not create SwScale context\n", .{});
        return error.SwsContextFailed;
    }
    defer c.sws_freeContext(sws_ctx);

    // Allocate frames
    const decoded_frame = c.av_frame_alloc();
    const gray_frame = c.av_frame_alloc();
    if (decoded_frame == null or gray_frame == null) {
        return error.FrameAllocFailed;
    }
    defer {
        c.av_frame_free(@constCast(&decoded_frame));
        c.av_frame_free(@constCast(&gray_frame));
    }

    gray_frame.*.format = @intCast(decoder_ctx.*.pix_fmt);
    gray_frame.*.width = decoder_ctx.*.width;
    gray_frame.*.height = decoder_ctx.*.height;
    if (c.av_frame_get_buffer(gray_frame, 0) < 0) {
        return error.FrameBufferAllocFailed;
    }

    std.debug.print("Converting to grayscale (HEVC, 1 Thread)...\n", .{});

    var packet = c.AVPacket{};
    var frame_count: u32 = 0;
    var packet_count: u32 = 0;
    var encoded_count: u32 = 0;

    // Read and process packets
    while (c.av_read_frame(input_ctx, &packet) >= 0) {
        defer c.av_packet_unref(&packet);

        if (packet.stream_index == video_stream_idx) {
            packet_count += 1;
            const send_ret = c.avcodec_send_packet(decoder_ctx, &packet);
            if (send_ret < 0 and send_ret != c.AVERROR(c.EAGAIN)) {
                continue;
            }

            while (c.avcodec_receive_frame(decoder_ctx, decoded_frame) >= 0) {
                if (c.av_frame_make_writable(gray_frame) < 0) continue;

                _ = c.sws_scale(
                    sws_ctx,
                    @ptrCast(&decoded_frame.*.data),
                    &decoded_frame.*.linesize,
                    0,
                    decoder_ctx.*.height,
                    @ptrCast(&gray_frame.*.data),
                    &gray_frame.*.linesize,
                );

                // --- Grayscale Logic ---
                const uv_width: usize = @intCast(@divTrunc(gray_frame.*.width, 2));
                const uv_height: usize = @intCast(@divTrunc(gray_frame.*.height, 2));
                const u_plane = gray_frame.*.data[1];
                const v_plane = gray_frame.*.data[2];
                const uv_linesize: usize = @intCast(gray_frame.*.linesize[1]);

                var y: usize = 0;
                while (y < uv_height) : (y += 1) {
                    const offset = y * uv_linesize;
                    @memset(u_plane[offset .. offset + uv_width], 128);
                    @memset(v_plane[offset .. offset + uv_width], 128);
                }

                // PTS Handling
                const input_pts = if (decoded_frame.*.pts != c.AV_NOPTS_VALUE)
                    decoded_frame.*.pts
                else if (decoded_frame.*.best_effort_timestamp != c.AV_NOPTS_VALUE)
                    decoded_frame.*.best_effort_timestamp
                else
                    decoded_frame.*.pkt_dts;

                gray_frame.*.pts = c.av_rescale_q(
                    input_pts,
                    video_stream.*.time_base,
                    encoder_ctx.*.time_base,
                );

                if (c.avcodec_send_frame(encoder_ctx, gray_frame) < 0) continue;

                var enc_packet = c.AVPacket{};
                while (c.avcodec_receive_packet(encoder_ctx, &enc_packet) >= 0) {
                    defer c.av_packet_unref(&enc_packet);
                    encoded_count += 1;

                    enc_packet.stream_index = 0;
                    c.av_packet_rescale_ts(
                        &enc_packet,
                        encoder_ctx.*.time_base,
                        out_video_stream.*.time_base,
                    );
                    _ = c.av_interleaved_write_frame(output_ctx, &enc_packet);
                }

                frame_count += 1;
                if (frame_count % 30 == 0) {
                    std.debug.print("\rProcessed {d} frames...", .{frame_count});
                }
            }
        } else if (audio_stream_idx != -1 and packet.stream_index == audio_stream_idx and out_audio_stream != null) {
            packet.stream_index = 1;
            c.av_packet_rescale_ts(
                &packet,
                audio_stream_ptr.?.*.time_base,
                out_audio_stream.?.*.time_base,
            );
            _ = c.av_interleaved_write_frame(output_ctx, &packet);
        }
    }

    _ = c.avcodec_send_frame(encoder_ctx, null);
    var enc_packet = c.AVPacket{};
    while (c.avcodec_receive_packet(encoder_ctx, &enc_packet) >= 0) {
        defer c.av_packet_unref(&enc_packet);
        encoded_count += 1;
        enc_packet.stream_index = 0;
        c.av_packet_rescale_ts(&enc_packet, encoder_ctx.*.time_base, out_video_stream.*.time_base);
        _ = c.av_interleaved_write_frame(output_ctx, &enc_packet);
    }

    _ = c.av_write_trailer(output_ctx);
    std.debug.print("\nDone. Saved to: {s}\n", .{output_path});
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();
    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    if (args.len != 3) {
        std.debug.print("Usage: {s} <input.mp4> <output.mp4>\n", .{args[0]});
        return error.InvalidArguments;
    }
    try convertToGrayscale(allocator, args[1], args[2]);
}
