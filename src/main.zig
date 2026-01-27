const std = @import("std");

const c = @cImport({
    @cInclude("libavformat/avformat.h");
    @cInclude("libavcodec/avcodec.h");
    @cInclude("libavutil/avutil.h");
});

fn extractAudio(allocator: std.mem.Allocator, input_path: []const u8, output_path: []const u8) !void {
    // Convert Zig strings to null-terminated C strings
    const input_cstr = try allocator.dupeZ(u8, input_path);
    defer allocator.free(input_cstr);
    const output_cstr = try allocator.dupeZ(u8, output_path);
    defer allocator.free(output_cstr);

    // Open input file
    var input_ctx: ?*c.AVFormatContext = null;
    if (c.avformat_open_input(&input_ctx, input_cstr.ptr, null, null) < 0) {
        std.debug.print("Error: Could not open input file '{s}'\n", .{input_path});
        return error.CannotOpenInput;
    }
    defer c.avformat_close_input(&input_ctx);

    std.debug.print("Opened input file: {s}\n", .{input_path});

    // Retrieve stream information
    if (c.avformat_find_stream_info(input_ctx, null) < 0) {
        std.debug.print("Error: Could not find stream information\n", .{});
        return error.StreamInfoNotFound;
    }

    std.debug.print("Found stream information\n", .{});

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
        std.debug.print("Error: Could not find audio stream in input file\n", .{});
        return error.NoAudioStream;
    }

    std.debug.print("Found audio stream at index {d}\n", .{audio_stream_idx});

    const audio_stream = input_ctx.?.streams[@intCast(audio_stream_idx)];
    const codec_params = audio_stream.*.codecpar;

    // Print audio stream information
    std.debug.print("Audio codec: {s}\n", .{c.avcodec_get_name(codec_params.*.codec_id)});
    std.debug.print("Sample rate: {d} Hz\n", .{codec_params.*.sample_rate});
    std.debug.print("Channels: {d}\n", .{codec_params.*.ch_layout.nb_channels});
    std.debug.print("Bit rate: {d} bps\n", .{codec_params.*.bit_rate});

    // Allocate output context
    var output_ctx: ?*c.AVFormatContext = null;
    if (c.avformat_alloc_output_context2(&output_ctx, null, null, output_cstr.ptr) < 0) {
        std.debug.print("Error: Could not create output context\n", .{});
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

    std.debug.print("Created output context for: {s}\n", .{output_path});

    // Create output audio stream
    const out_stream = c.avformat_new_stream(output_ctx, null);
    if (out_stream == null) {
        std.debug.print("Error: Failed to create output stream\n", .{});
        return error.CannotCreateOutputStream;
    }

    // Copy codec parameters from input to output
    if (c.avcodec_parameters_copy(out_stream.*.codecpar, codec_params) < 0) {
        std.debug.print("Error: Failed to copy codec parameters\n", .{});
        return error.CannotCopyCodecParams;
    }

    out_stream.*.codecpar.*.codec_tag = 0;

    // Open output file
    if ((output_ctx.?.oformat.*.flags & c.AVFMT_NOFILE) == 0) {
        if (c.avio_open(&output_ctx.?.pb, output_cstr.ptr, c.AVIO_FLAG_WRITE) < 0) {
            std.debug.print("Error: Could not open output file '{s}'\n", .{output_path});
            return error.CannotOpenOutput;
        }
    }

    // Write output file header
    if (c.avformat_write_header(output_ctx, null) < 0) {
        std.debug.print("Error: Failed to write output header\n", .{});
        return error.CannotWriteHeader;
    }

    std.debug.print("Writing audio packets...\n", .{});

    // Read and write packets
    var packet: c.AVPacket = undefined;
    var packet_count: usize = 0;

    while (true) {
        const ret = c.av_read_frame(input_ctx, &packet);
        if (ret < 0) {
            if (ret == c.AVERROR_EOF) {
                break; // End of file
            }
            std.debug.print("Error reading frame\n", .{});
            return error.ReadFrameError;
        }
        defer c.av_packet_unref(&packet);

        // Only process audio stream packets
        if (packet.stream_index != audio_stream_idx) {
            continue;
        }

        packet_count += 1;

        // Rescale packet timestamps
        packet.stream_index = 0; // Output stream index
        c.av_packet_rescale_ts(
            &packet,
            audio_stream.*.time_base,
            out_stream.*.time_base,
        );

        // Write packet to output
        if (c.av_interleaved_write_frame(output_ctx, &packet) < 0) {
            std.debug.print("Error writing packet\n", .{});
            return error.WritePacketError;
        }
    }

    std.debug.print("Wrote {d} audio packets\n", .{packet_count});

    // Write output file trailer
    if (c.av_write_trailer(output_ctx) < 0) {
        std.debug.print("Error: Failed to write output trailer\n", .{});
        return error.CannotWriteTrailer;
    }

    std.debug.print("Successfully extracted audio to: {s}\n", .{output_path});
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    // Print FFmpeg version info
    std.debug.print("=== FFmpeg Audio Extractor ===\n", .{});
    const version = c.av_version_info();
    std.debug.print("FFmpeg version: {s}\n\n", .{version});

    // Get command line arguments
    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    if (args.len != 3) {
        std.debug.print("Usage: {s} <input.mp4> <output.aac>\n", .{args[0]});
        std.debug.print("Example: {s} video.mp4 audio.aac\n", .{args[0]});
        std.debug.print("\nSupported output formats:\n", .{});
        std.debug.print("  .aac  - AAC audio\n", .{});
        std.debug.print("  .mp3  - MP3 audio\n", .{});
        std.debug.print("  .m4a  - M4A audio\n", .{});
        std.debug.print("  .wav  - WAV audio\n", .{});
        std.debug.print("  .ogg  - OGG audio\n", .{});
        return error.InvalidArguments;
    }

    const input_file = args[1];
    const output_file = args[2];

    std.debug.print("Input:  {s}\n", .{input_file});
    std.debug.print("Output: {s}\n\n", .{output_file});

    try extractAudio(allocator, input_file, output_file);

    std.debug.print("\n=== Extraction Complete ===\n", .{});
}
