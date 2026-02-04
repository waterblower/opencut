const std = @import("std");
const zaudio = @import("zaudio");
const print = std.debug.print;
const default_audio_data = @import("default_audio").default_audio_data;

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    // Get command line arguments
    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    const use_file = args.len == 2;
    const audio_file_arg = if (use_file) args[1] else null;

    if (audio_file_arg) |arg| {
        std.debug.print("Playing audio file: {s}\n", .{arg});
    } else {
        std.debug.print("Playing embedded audio (test.mp3)\n", .{});
    }
    std.debug.print("Press Ctrl+C to stop...\n\n", .{});

    // Initialize zaudio
    zaudio.init(allocator);
    defer zaudio.deinit();

    // Create audio engine
    const engine = try zaudio.Engine.create(null);
    defer engine.destroy();

    // Load and play the sound based on whether a file argument was provided
    const sound = try loadSound(engine, audio_file_arg);
    defer sound.destroy();

    // Get sound duration
    const length_in_pcm_frames = try sound.getLengthInPcmFrames();
    const length_in_seconds = try sound.getLengthInSeconds();

    std.debug.print("Duration: {d:.2}s ({} PCM frames)\n", .{ length_in_seconds, length_in_pcm_frames });

    // Start playback
    try sound.start();

    // Wait for playback to finish
    const start_time = std.time.milliTimestamp();
    var last_cursor: u64 = 0;

    while (true) {
        // Check if sound is still playing
        if (!sound.isPlaying()) {
            break;
        }

        // Get current playback position
        const cursor_in_pcm_frames = try sound.getCursorInPcmFrames();
        const cursor_in_seconds = try sound.getCursorInSeconds();

        // Update progress display
        if (cursor_in_pcm_frames != last_cursor) {
            const progress = (cursor_in_seconds / length_in_seconds) * 100.0;
            const bar_width = 40;
            const filled = @as(usize, @intFromFloat((cursor_in_seconds / length_in_seconds) * @as(f32, @floatFromInt(bar_width))));

            std.debug.print("\r[", .{});
            var i: usize = 0;
            while (i < bar_width) : (i += 1) {
                if (i < filled) {
                    std.debug.print("=", .{});
                } else if (i == filled) {
                    std.debug.print(">", .{});
                } else {
                    std.debug.print(" ", .{});
                }
            }
            std.debug.print("] {d:.1}% ({d:.1}s / {d:.1}s)", .{ progress, cursor_in_seconds, length_in_seconds });

            last_cursor = cursor_in_pcm_frames;
        }

        // Sleep a bit to avoid busy waiting
        std.Thread.sleep(100 * std.time.ns_per_ms);

        // Timeout after expected duration + 1 second
        const elapsed = std.time.milliTimestamp() - start_time;
        if (elapsed > (@as(i64, @intFromFloat(length_in_seconds * 1000)) + 1000)) {
            break;
        }
    }

    std.debug.print("\n\nPlayback finished.\n", .{});
}

fn loadSound(engine: *zaudio.Engine, file_path: ?[:0]const u8) !*zaudio.Sound {
    if (file_path) |path| {
        return engine.createSoundFromFile(
            path,
            .{
                .flags = .{ .stream = true },
                .sgroup = null,
                .done_fence = null,
            },
        );
    }

    const decoder_config = zaudio.Decoder.Config.initDefault();
    const decoder = try zaudio.Decoder.createFromMemory(
        default_audio_data.ptr,
        default_audio_data.len,
        decoder_config,
    );
    errdefer decoder.destroy();
    // Convert decoder to data source
    const data_source = decoder.asDataSourceMut();
    return engine.createSoundFromDataSource(
        data_source,
        .{ .stream = true },
        null,
    );
}
