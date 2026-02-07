const std = @import("std");
const print = std.debug.print;
const now = std.time.milliTimestamp;
const dvui = @import("dvui");
const SDLBackend = @import("SDLBackend");
const vid = @import("vid.zig");
const builtin = @import("builtin");
const fs = @import("fs.zig");
const ui = @import("ui.zig");

// Use SDL from the backend
const SDL = SDLBackend.c;

var gpa_instance = std.heap.GeneralPurposeAllocator(.{}){};
const gpa = gpa_instance.allocator();

// Rendering state
var g_backend: ?*SDLBackend = null;

// Playback state

var g_last_frame_time: u64 = 0;

const State = struct {
    is_playing: bool,
};

pub fn main() !void {
    var state: State = .{ .is_playing = true };
    defer _ = gpa_instance.deinit();

    // Parse command line arguments
    const args = try std.process.argsAlloc(gpa);
    defer std.process.argsFree(gpa, args);

    for (args) |arg| {
        std.debug.print("Arg: {s}\n", .{arg});
    }

    const video_path = if (args.len > 1)
        args[1]
    else
        "test-videos/test1.mp4";

    // Initialize video
    const v = vid.openVideo(gpa, video_path) catch |err| blk: {
        std.debug.print("Could not open video: {}\n", .{err});
        break :blk null;
    };
    if (v == null) {
        std.debug.panic("video is null", .{});
    }
    var video: *vid.Video = v orelse unreachable;
    defer {
        video.deinit();
    }

    // Initialize SDL backend
    var backend = try SDLBackend.initWindow(.{
        .allocator = gpa,
        .size = .{ .w = 1024.0, .h = 768.0 },
        .min_size = .{ .w = 640.0, .h = 480.0 },
        .vsync = true,
        .title = "DVUI Video Player",
    });
    g_backend = &backend;
    defer backend.deinit();

    // Create texture after video was loaded
    const texture = try createTexture(backend.renderer, video.width, video.height);
    defer {
        SDL.SDL_DestroyTexture(texture);
    }

    // Initialize dvui window
    var win = try dvui.Window.init(@src(), gpa, backend.backend(), .{});
    defer win.deinit();

    var interrupted = false;

    var arena = std.heap.ArenaAllocator.init(gpa);
    defer arena.deinit();

    while (true) {
        const aa = arena.allocator();
        defer _ = arena.reset(.retain_capacity);

        const nstime = win.beginWait(interrupted);

        try win.begin(nstime);

        try backend.addAllEvents(&win);

        // Clear background
        _ = SDL.SDL_SetRenderDrawColor(backend.renderer, 0, 0, 0, 255);
        _ = SDL.SDL_RenderClear(backend.renderer);

        // Update video frame if playing

        const t0 = std.time.milliTimestamp();
        try updateNextFrame(video, texture, state.is_playing);
        print("updateNextFrame cost: {d}\n", .{(std.time.milliTimestamp() - t0)});

        // Render GUI
        // In your main loop or parent function:
        if (try ui.dialogs(aa)) |path| {
            // This block ONLY runs if:
            // 1. Button was clicked
            // 2. Dialog succeeded
            // 3. User picked a path (didn't cancel)
            std.debug.print("User selected: {s}\n", .{path});
        }

        const t1 = std.time.milliTimestamp();
        const keep_running = guiFrame(&backend, video, texture, &state);
        if (!keep_running) break;
        print("\rguiFrame cost: {d}\n", .{(std.time.milliTimestamp() - t1)});

        const end_micros = try win.end(.{});

        try backend.setCursor(win.cursorRequested());
        try backend.textInputRect(win.textInputRequested());

        try backend.renderPresent();

        // When video is playing, we need continuous updates
        var wait_event_micros = win.waitTime(end_micros);
        if (state.is_playing and !video.isFinished()) {
            // Limit wait to frame duration to ensure continuous playback
            const frame_duration_ms = video.frameDurationMs();
            const max_wait_micros = frame_duration_ms * 1000;
            if (wait_event_micros > max_wait_micros) {
                wait_event_micros = max_wait_micros;
            }
        }
        interrupted = try backend.waitEventTimeout(wait_event_micros);
    }
}

fn createTexture(renderer: *SDL.SDL_Renderer, width: i32, height: i32) !*SDL.SDL_Texture {
    const texture = SDL.SDL_CreateTexture(
        renderer,
        SDL.SDL_PIXELFORMAT_ARGB8888,
        SDL.SDL_TEXTUREACCESS_STREAMING,
        width,
        height,
    );
    if (texture == null) {
        return error.CouldNotCreateTexture;
    }
    return texture.?;
}

fn updateNextFrame(video: *vid.Video, texture: *SDL.SDL_Texture, is_playing: bool) !void {
    // const t0 = std.time.milliTimestamp();
    if (!is_playing or video.isFinished()) {
        return;
    }

    const current_time = SDL.SDL_GetTicks();

    // Initialize timing on first frame
    if (g_last_frame_time == 0) {
        g_last_frame_time = current_time;
    }

    const frame_duration_ms = video.frameDurationMs();

    if (current_time - g_last_frame_time < frame_duration_ms) {
        return;
    }

    // Advance by frame duration, not wall clock time
    g_last_frame_time += frame_duration_ms;

    // If we've fallen too far behind (more than 1 second), resync to current time
    if (current_time > g_last_frame_time + 1000) {
        g_last_frame_time = current_time;
    }

    // Use internal video buffer and update texture
    const pixels = video.rgb_frame.*.data[0];
    const pitch = video.rgb_frame.*.linesize[0];

    if (pixels) |p| {
        const dest = @as([*]u8, @ptrCast(p));
        const t = now();
        _ = video.renderNextFrame(dest, pitch) catch |err| {
            print("Render error: {}\n", .{err});
        };
        print("renderNextFrame: {d}\n", .{now() - t});

        _ = SDL.SDL_UpdateTexture(texture, null, p, pitch);
        print("SDL_UpdateTexture: {d}\n", .{now() - t});
    }
}

fn guiFrame(backend: *SDLBackend, video: *vid.Video, texture: *SDL.SDL_Texture, state: *State) bool {
    // Top menu bar
    {
        var hbox = dvui.box(@src(), .{ .dir = .horizontal }, .{ .style = .window, .background = true, .expand = .horizontal });
        defer hbox.deinit();

        var m = dvui.menu(@src(), .horizontal, .{});
        defer m.deinit();

        if (dvui.menuItemLabel(@src(), "File", .{ .submenu = true }, .{})) |r| {
            var fw = dvui.floatingMenu(@src(), .{ .from = r }, .{});
            defer fw.deinit();

            if (dvui.menuItemLabel(@src(), "Exit", .{}, .{ .expand = .horizontal }) != null) {
                return false;
            }
        }
    }

    // Main content area
    var scroll = dvui.scrollArea(@src(), .{}, .{ .expand = .both });
    defer scroll.deinit();

    // Title
    var tl = dvui.textLayout(@src(), .{}, .{ .expand = .horizontal, .font = .theme(.title) });
    tl.addText("DVUI Video Player", .{});
    tl.deinit();

    // Info text
    var info = dvui.textLayout(@src(), .{}, .{ .expand = .horizontal, .margin = .{ .y = 10 } });
    info.addText("Video only playback (no audio)\n", .{});

    const status = if (state.is_playing) "Playing" else if (video.isFinished()) "Finished" else "Paused";
    var buf: [256]u8 = undefined;
    const text = std.fmt.bufPrint(&buf, "Resolution: {}x{} | FPS: {d:.1} | Status: {s}\n\n", .{ video.width, video.height, video.fps(), status }) catch "Error formatting";
    info.addText(text, .{});

    info.deinit();

    // Video display area

    var video_box = dvui.box(@src(), .{}, .{
        .expand = .horizontal,
        .min_size_content = .{ .h = 400 },
        .background = true,
        .margin = .{ .x = 8, .w = 8, .y = 8, .h = 8 },
    });
    defer video_box.deinit();

    // Get the screen rectangle for the box
    const rs = video_box.data().contentRectScale();

    // Calculate aspect ratio preserving destination rectangle
    const frame_aspect = @as(f32, @floatFromInt(video.width)) / @as(f32, @floatFromInt(video.height));
    const box_aspect = rs.r.w / rs.r.h;

    var dst_rect: SDL.SDL_FRect = undefined;

    if (box_aspect > frame_aspect) {
        // Box is wider than frame - fit to height
        dst_rect.h = rs.r.h;
        dst_rect.w = dst_rect.h * frame_aspect;
        dst_rect.x = rs.r.x + (rs.r.w - dst_rect.w) / 2.0;
        dst_rect.y = rs.r.y;
    } else {
        // Box is taller than frame - fit to width
        dst_rect.w = rs.r.w;
        dst_rect.h = dst_rect.w / frame_aspect;
        dst_rect.x = rs.r.x;
        dst_rect.y = rs.r.y + (rs.r.h - dst_rect.h) / 2.0;
    }

    // Render video texture
    _ = SDL.SDL_RenderTexture(backend.renderer, texture, null, &dst_rect);

    // Playback controls
    {
        var controls = dvui.box(@src(), .{ .dir = .horizontal }, .{ .expand = .horizontal, .margin = .{ .y = 10 } });
        defer controls.deinit();

        if (!video.isFinished()) {
            const button_label = if (state.is_playing) "Pause" else "Play";
            if (dvui.button(@src(), button_label, .{}, .{})) {
                state.is_playing = !state.is_playing;
                if (state.is_playing) {
                    g_last_frame_time = SDL.SDL_GetTicks();
                }
            }
        }

        if (video.isFinished()) {
            if (dvui.button(@src(), "Restart", .{}, .{})) {
                video.restart() catch |err| {
                    std.debug.print("Error restarting video: {}\n", .{err});
                };
                state.is_playing = true;
                g_last_frame_time = SDL.SDL_GetTicks();
            }
        }
    }

    if (dvui.button(@src(), "Debug Window", .{}, .{ .margin = .{ .y = 10 } })) {
        dvui.toggleDebugWindow();
    }

    // Request continuous rendering when video is playing
    if (state.is_playing and !video.isFinished()) {
        dvui.refresh(null, @src(), null);
    }

    // Check for quit events
    for (dvui.events()) |*e| {
        if (e.evt == .window and e.evt.window.action == .close) return false;
        if (e.evt == .app and e.evt.app.action == .quit) return false;
    }

    return true;
}
