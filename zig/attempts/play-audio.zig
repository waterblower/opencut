const std = @import("std");
const zaudio = @import("zaudio");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    // Get command line arguments
    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    if (args.len != 2) {
        std.debug.print("Usage: {s} <audio-file>\n", .{args[0]});
        std.debug.print("Example: {s} input.mp4\n", .{args[0]});
        return error.InvalidArguments;
    }

    const audio_file = args[1];

    // Check if file exists
    const file = std.fs.cwd().openFile(audio_file, .{}) catch |err| {
        std.debug.print("Error: Cannot open file '{s}': {}\n", .{ audio_file, err });
        return err;
    };
    file.close();

    std.debug.print("Playing audio: {s}\n", .{audio_file});
    std.debug.print("Press Ctrl+C to stop...\n\n", .{});

    // Initialize zaudio
    zaudio.init(allocator);
    defer zaudio.deinit();

    // Create audio engine
    const engine = try zaudio.Engine.create(null);
    defer engine.destroy();

    // Load and play the sound
    // Note: MP4 containers may not be directly supported by miniaudio
    // For MP4 files, you may need to extract the audio stream first
    const sound = engine.createSoundFromFile(
        audio_file,
        .{ .flags = .{ .stream = true } },
    ) catch |err| {
        std.debug.print("Error loading audio file: {}\n", .{err});
        std.debug.print("\nNote: MP4 containers may not be directly supported.\n", .{});
        std.debug.print("Try using a supported format like:\n", .{});
        std.debug.print("  - WAV (.wav)\n", .{});
        std.debug.print("  - MP3 (.mp3)\n", .{});
        std.debug.print("  - FLAC (.flac)\n", .{});
        std.debug.print("  - OGG Vorbis (.ogg)\n", .{});
        std.debug.print("\nOr extract audio from MP4 using:\n", .{});
        std.debug.print("  ffmpeg -i input.mp4 -vn -acodec libmp3lame output.mp3\n", .{});
        return err;
    };
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
