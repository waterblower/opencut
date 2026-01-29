const std = @import("std");

const c = @cImport({
    @cInclude("libavformat/avformat.h");
    @cInclude("libavcodec/avcodec.h");
    @cInclude("libavutil/avutil.h");
});

/// Simple add function that can be called from Flutter via FFI
/// This uses the C calling convention to be compatible with FFI
export fn add(a: i32, b: i32) i32 {
    return a + b;
}

/// Global allocator for FFI functions
var gpa = std.heap.GeneralPurposeAllocator(.{}){};

/// Get video information from an MP4 file
/// Returns a C string containing video metadata (must be freed by caller using free_string)
/// Returns null if there's an error
export fn get_video_information(file_path: [*:0]const u8) ?[*:0]const u8 {
    const allocator = gpa.allocator();

    // Open input file
    var fmt_ctx: ?*c.AVFormatContext = null;
    if (c.avformat_open_input(&fmt_ctx, file_path, null, null) < 0) {
        return createErrorString(allocator, "Failed to open video file") catch null;
    }
    defer c.avformat_close_input(&fmt_ctx);

    // Retrieve stream information
    if (c.avformat_find_stream_info(fmt_ctx, null) < 0) {
        return createErrorString(allocator, "Failed to retrieve stream information") catch null;
    }

    // Find video stream
    var video_stream_idx: i32 = -1;
    var video_stream: ?*c.AVStream = null;

    const nb_streams = fmt_ctx.?.*.nb_streams;
    var i: u32 = 0;
    while (i < nb_streams) : (i += 1) {
        const stream = fmt_ctx.?.*.streams[i];
        if (stream.*.codecpar.*.codec_type == c.AVMEDIA_TYPE_VIDEO) {
            video_stream_idx = @intCast(i);
            video_stream = stream;
            break;
        }
    }

    if (video_stream_idx == -1) {
        return createErrorString(allocator, "No video stream found") catch null;
    }

    // Extract video information
    const codecpar = video_stream.?.*.codecpar;
    const width = codecpar.*.width;
    const height = codecpar.*.height;
    const codec_id = codecpar.*.codec_id;

    // Get codec name
    const codec = c.avcodec_find_decoder(codec_id);
    const codec_name = if (codec != null)
        std.mem.span(codec.?.*.name)
    else
        "unknown";

    // Calculate FPS
    const avg_frame_rate = video_stream.?.*.avg_frame_rate;
    const fps = if (avg_frame_rate.den != 0)
        @as(f64, @floatFromInt(avg_frame_rate.num)) / @as(f64, @floatFromInt(avg_frame_rate.den))
    else
        0.0;

    // Calculate duration and frame count
    const duration_seconds = if (fmt_ctx.?.*.duration != c.AV_NOPTS_VALUE)
        @as(f64, @floatFromInt(fmt_ctx.?.*.duration)) / @as(f64, @floatFromInt(c.AV_TIME_BASE))
    else
        0.0;

    const frame_count = if (video_stream.?.*.nb_frames != 0)
        video_stream.?.*.nb_frames
    else if (fps > 0)
        @as(i64, @intFromFloat(duration_seconds * fps))
    else
        0;

    // Get file size
    const file_size = getFileSize(file_path) catch 0;

    // Get bitrate
    const bit_rate = if (fmt_ctx.?.*.bit_rate != 0)
        fmt_ctx.?.*.bit_rate
    else if (codecpar.*.bit_rate != 0)
        codecpar.*.bit_rate
    else
        0;

    // Get pixel format
    const pix_fmt = codecpar.*.format;

    // Build the information string
    const info_string = std.fmt.allocPrint(allocator,
        \\Video Information:
        \\Resolution: {d}x{d}
        \\FPS: {d:.2}
        \\Frame Count: {d}
        \\Duration: {d:.2} seconds
        \\Codec: {s}
        \\Pixel Format: {d}
        \\Bitrate: {d} kb/s
        \\File Size: {d:.2} MB
    , .{
        width,
        height,
        fps,
        frame_count,
        duration_seconds,
        codec_name,
        pix_fmt,
        @divTrunc(bit_rate, 1000),
        @as(f64, @floatFromInt(file_size)) / (1024.0 * 1024.0),
    }) catch {
        return createErrorString(allocator, "Failed to format video information") catch null;
    };

    // Add null terminator
    const null_terminated = allocator.dupeZ(u8, info_string) catch {
        allocator.free(info_string);
        return createErrorString(allocator, "Failed to allocate null-terminated string") catch null;
    };
    allocator.free(info_string);

    return null_terminated.ptr;
}

/// Free a string allocated by get_video_information
export fn free_string(str: [*:0]const u8) void {
    const allocator = gpa.allocator();
    const len = std.mem.len(str);
    const slice = str[0..len :0];
    allocator.free(slice);
}

/// Helper function to get file size
fn getFileSize(file_path: [*:0]const u8) !u64 {
    const path_slice = std.mem.span(file_path);
    const file = try std.fs.cwd().openFile(path_slice, .{});
    defer file.close();
    const stat = try file.stat();
    return stat.size;
}

/// Helper function to create error strings
fn createErrorString(allocator: std.mem.Allocator, message: []const u8) !?[*:0]const u8 {
    const error_string = try std.fmt.allocPrint(allocator, "Error: {s}", .{message});
    const null_terminated = try allocator.dupeZ(u8, error_string);
    allocator.free(error_string);
    return null_terminated.ptr;
}
