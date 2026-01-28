const std = @import("std");

const c = @cImport({
    @cInclude("libavformat/avformat.h");
    @cInclude("libavcodec/avcodec.h");
    @cInclude("libavutil/avutil.h");
});

fn extractAudio(input_path: [*:0]const u8, output_path: [*:0]const u8) !void {
    // Open input file
    var input_ctx: ?*c.AVFormatContext = null;
    if (c.avformat_open_input(&input_ctx, input_path, null, null) < 0) {
        return error.CannotOpenInput;
    }
    defer c.avformat_close_input(&input_ctx);

    // Retrieve stream information
    if (c.avformat_find_stream_info(input_ctx, null) < 0) {
        return error.StreamInfoNotFound;
    }

    // Find the audio stream
    var audio_stream_idx: c_int = -1;
    const nb_streams = input_ctx.?.nb_streams;

    var i: u32 = 0;
    while (i < nb_streams) : (i += 1) {
        const stream = input_ctx.?.streams[i];
        if (stream.*.codecpar.*.codec_type == c.AVMEDIA_TYPE_AUDIO) {
            audio_stream_idx = @intCast(i);
            break;
        }
    }

    if (audio_stream_idx == -1) {
        return error.NoAudioStream;
    }

    const audio_stream = input_ctx.?.streams[@intCast(audio_stream_idx)];
    const codec_params = audio_stream.*.codecpar;

    // Allocate output context
    var output_ctx: ?*c.AVFormatContext = null;
    if (c.avformat_alloc_output_context2(&output_ctx, null, null, output_path) < 0) {
        return error.CannotCreateOutputContext;
    }
    defer {
        if (output_ctx) |ctx| {
            if ((ctx.oformat.*.flags & c.AVFMT_NOFILE) == 0) {
                _ = c.avio_closep(&ctx.pb);
            }
            c.avformat_free_context(ctx);
        }
    }

    // Create output audio stream
    const out_stream = c.avformat_new_stream(output_ctx, null);
    if (out_stream == null) {
        return error.CannotCreateOutputStream;
    }

    // Copy codec parameters from input to output
    if (c.avcodec_parameters_copy(out_stream.*.codecpar, codec_params) < 0) {
        return error.CannotCopyCodecParams;
    }

    out_stream.*.codecpar.*.codec_tag = 0;

    // Open output file
    if ((output_ctx.?.oformat.*.flags & c.AVFMT_NOFILE) == 0) {
        if (c.avio_open(&output_ctx.?.pb, output_path, c.AVIO_FLAG_WRITE) < 0) {
            return error.CannotOpenOutput;
        }
    }

    // Write output file header
    if (c.avformat_write_header(output_ctx, null) < 0) {
        return error.CannotWriteHeader;
    }

    // Read and write packets - optimized hot loop
    var packet: c.AVPacket = undefined;
    const audio_idx = audio_stream_idx;
    const in_time_base = audio_stream.*.time_base;
    const out_time_base = out_stream.*.time_base;

    while (c.av_read_frame(input_ctx, &packet) >= 0) {
        // Only process audio stream packets
        if (packet.stream_index == audio_idx) {
            // Rescale packet timestamps
            packet.stream_index = 0;
            c.av_packet_rescale_ts(&packet, in_time_base, out_time_base);

            // Write packet to output
            _ = c.av_write_frame(output_ctx, &packet);
        }

        c.av_packet_unref(&packet);
    }

    // Write output file trailer
    _ = c.av_write_trailer(output_ctx);
}

pub fn main() !void {
    // Use C allocator (simpler, potentially faster for this use case)
    const allocator = std.heap.c_allocator;

    // Suppress FFmpeg logging
    c.av_log_set_level(c.AV_LOG_QUIET);

    // Get command line arguments
    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    if (args.len != 3) {
        return error.InvalidArguments;
    }

    // Convert to null-terminated strings
    const input_cstr = try allocator.dupeZ(u8, args[1]);
    defer allocator.free(input_cstr);
    const output_cstr = try allocator.dupeZ(u8, args[2]);
    defer allocator.free(output_cstr);

    try extractAudio(input_cstr.ptr, output_cstr.ptr);
}
