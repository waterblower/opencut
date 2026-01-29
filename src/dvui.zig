const std = @import("std");
const dvui = @import("dvui");
const SDLBackend = @import("SDLBackend");

const c = @cImport({
    @cInclude("libavformat/avformat.h");
    @cInclude("libavcodec/avcodec.h");
    @cInclude("libavutil/avutil.h");
    @cInclude("libavutil/imgutils.h");
    @cInclude("libswscale/swscale.h");
});

// Use SDL from the backend
const SDL = SDLBackend.c;

var gpa_instance = std.heap.GeneralPurposeAllocator(.{}){};
const gpa = gpa_instance.allocator();

// Video state
var g_fmt_ctx: ?*c.AVFormatContext = null;
var g_codec_ctx: ?*c.AVCodecContext = null;
var g_frame: ?*c.AVFrame = null;
var g_rgb_frame: ?*c.AVFrame = null;
var g_packet: ?*c.AVPacket = null;
var g_sws_ctx: ?*c.struct_SwsContext = null;
var g_video_stream_idx: c_int = -1;
var g_video_width: c_int = 0;
var g_video_height: c_int = 0;

// Rendering state
var g_texture: ?*SDL.SDL_Texture = null;
var g_backend: ?*SDLBackend = null;

// Playback state
var g_is_playing: bool = true; // Auto-start playback
var g_video_finished: bool = false;
var g_frame_duration_ms: u32 = 33;
var g_last_frame_time: u64 = 0;

pub fn main() !void {
    defer _ = gpa_instance.deinit();

    // Initialize video decoder
    initVideoDecoder("test-videos/test.mp4") catch |err| {
        std.debug.print("Could not initialize video decoder: {}\n", .{err});
        std.debug.print("Make sure 'test.mp4' exists in the current directory\n", .{});
        // Continue anyway to show the GUI
    };
    defer cleanupVideo();

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

    // Create texture if video was loaded
    if (g_video_width > 0) {
        try createTexture(backend.renderer);
    }

    // Initialize dvui window
    var win = try dvui.Window.init(@src(), gpa, backend.backend(), .{});
    defer win.deinit();

    var interrupted = false;

    // Main loop
    while (true) {
        const nstime = win.beginWait(interrupted);

        try win.begin(nstime);

        try backend.addAllEvents(&win);

        // Clear background
        _ = SDL.SDL_SetRenderDrawColor(backend.renderer, 0, 0, 0, 255);
        _ = SDL.SDL_RenderClear(backend.renderer);

        // Update video frame if playing
        updateNextFrame();

        // Render GUI
        const keep_running = guiFrame(&backend);
        if (!keep_running) break;

        const end_micros = try win.end(.{});

        try backend.setCursor(win.cursorRequested());
        try backend.textInputRect(win.textInputRequested());

        try backend.renderPresent();

        // When video is playing, we need continuous updates
        // Calculate wait time, but cap it at frame duration when playing
        var wait_event_micros = win.waitTime(end_micros);
        if (g_is_playing and !g_video_finished) {
            // Limit wait to frame duration to ensure continuous playback
            const max_wait_micros = g_frame_duration_ms * 1000;
            if (wait_event_micros > max_wait_micros) {
                wait_event_micros = max_wait_micros;
            }
        }
        interrupted = try backend.waitEventTimeout(wait_event_micros);
    }
}

fn initVideoDecoder(video_path: [*c]const u8) !void {
    // Open video file
    if (c.avformat_open_input(&g_fmt_ctx, video_path, null, null) < 0) {
        return error.CouldNotOpenFile;
    }

    // Retrieve stream information
    if (c.avformat_find_stream_info(g_fmt_ctx, null) < 0) {
        return error.CouldNotFindStreamInfo;
    }

    // Find the first video stream
    var i: c_uint = 0;
    while (i < g_fmt_ctx.?.nb_streams) : (i += 1) {
        if (g_fmt_ctx.?.streams[i].*.codecpar.*.codec_type == c.AVMEDIA_TYPE_VIDEO) {
            g_video_stream_idx = @intCast(i);
            break;
        }
    }

    if (g_video_stream_idx == -1) {
        return error.NoVideoStream;
    }

    // Get codec parameters
    const codecpar = g_fmt_ctx.?.streams[@intCast(g_video_stream_idx)].*.codecpar;

    // Find decoder
    const codec = c.avcodec_find_decoder(codecpar.*.codec_id);
    if (codec == null) {
        return error.UnsupportedCodec;
    }

    // Allocate codec context
    g_codec_ctx = c.avcodec_alloc_context3(codec);
    if (g_codec_ctx == null) {
        return error.CouldNotAllocateCodecContext;
    }

    // Copy codec parameters to context
    if (c.avcodec_parameters_to_context(g_codec_ctx, codecpar) < 0) {
        return error.CouldNotCopyCodecParams;
    }

    // Open codec
    if (c.avcodec_open2(g_codec_ctx, codec, null) < 0) {
        return error.CouldNotOpenCodec;
    }

    g_video_width = g_codec_ctx.?.*.width;
    g_video_height = g_codec_ctx.?.*.height;

    // Calculate frame duration (default to 30fps if we can't determine)
    const stream = g_fmt_ctx.?.streams[@intCast(g_video_stream_idx)];
    var fps: f64 = 30.0;
    if (stream.*.avg_frame_rate.den > 0) {
        const calc_fps = @as(f64, @floatFromInt(stream.*.avg_frame_rate.num)) / @as(f64, @floatFromInt(stream.*.avg_frame_rate.den));
        if (calc_fps > 0) {
            fps = calc_fps;
            g_frame_duration_ms = @intFromFloat(1000.0 / fps);
        }
    }

    std.debug.print("Video: {}x{} @ {d:.2} fps (frame duration: {}ms)\n", .{ g_video_width, g_video_height, fps, g_frame_duration_ms });

    // Allocate frame
    g_frame = c.av_frame_alloc();
    if (g_frame == null) {
        return error.CouldNotAllocateFrame;
    }

    // Allocate RGB frame
    g_rgb_frame = c.av_frame_alloc();
    if (g_rgb_frame == null) {
        return error.CouldNotAllocateRGBFrame;
    }

    // Allocate packet
    g_packet = c.av_packet_alloc();
    if (g_packet == null) {
        return error.CouldNotAllocatePacket;
    }

    // Initialize SWS context for color conversion
    g_sws_ctx = c.sws_getContext(
        g_video_width,
        g_video_height,
        g_codec_ctx.?.*.pix_fmt,
        g_video_width,
        g_video_height,
        c.AV_PIX_FMT_RGB24,
        c.SWS_BILINEAR,
        null,
        null,
        null,
    );
    if (g_sws_ctx == null) {
        return error.CouldNotInitSWSContext;
    }

    // Allocate buffer for RGB frame
    const num_bytes = c.av_image_get_buffer_size(c.AV_PIX_FMT_RGB24, g_video_width, g_video_height, 1);
    const buffer = c.av_malloc(@intCast(num_bytes));
    if (buffer == null) {
        return error.CouldNotAllocateBuffer;
    }

    _ = c.av_image_fill_arrays(
        @ptrCast(&g_rgb_frame.?.*.data),
        @ptrCast(&g_rgb_frame.?.*.linesize),
        @ptrCast(buffer),
        c.AV_PIX_FMT_RGB24,
        g_video_width,
        g_video_height,
        1,
    );
}

fn createTexture(renderer: *SDL.SDL_Renderer) !void {
    g_texture = SDL.SDL_CreateTexture(
        renderer,
        SDL.SDL_PIXELFORMAT_RGB24,
        SDL.SDL_TEXTUREACCESS_STREAMING,
        g_video_width,
        g_video_height,
    );
    if (g_texture == null) {
        return error.CouldNotCreateTexture;
    }
}

fn readAndDispatchPacket() bool {
    if (g_fmt_ctx == null or g_packet == null) {
        return false;
    }

    if (c.av_read_frame(g_fmt_ctx, g_packet) < 0) {
        return false;
    }

    const stream_idx = g_packet.?.*.stream_index;

    if (stream_idx == g_video_stream_idx and g_codec_ctx != null) {
        _ = c.avcodec_send_packet(g_codec_ctx, g_packet);
    }

    c.av_packet_unref(g_packet);
    return true;
}

fn updateNextFrame() void {
    if (g_video_finished or !g_is_playing or g_codec_ctx == null or
        g_frame == null or g_rgb_frame == null or
        g_sws_ctx == null or g_texture == null)
    {
        return;
    }

    const current_time = SDL.SDL_GetTicks();

    // Initialize timing on first frame
    if (g_last_frame_time == 0) {
        g_last_frame_time = current_time;
    }

    if (current_time - g_last_frame_time < g_frame_duration_ms) {
        return;
    }

    // Advance by frame duration, not wall clock time
    // This ensures consistent playback speed
    g_last_frame_time += g_frame_duration_ms;

    // If we've fallen too far behind (more than 1 second), resync to current time
    if (current_time > g_last_frame_time + 1000) {
        g_last_frame_time = current_time;
    }

    // Try to receive a decoded video frame
    var ret = c.avcodec_receive_frame(g_codec_ctx, g_frame);

    // If we need more data, read packets until we get a video frame
    while (ret == c.AVERROR(c.EAGAIN)) {
        if (!readAndDispatchPacket()) {
            // End of file
            g_video_finished = true;
            g_is_playing = false;
            return;
        }
        ret = c.avcodec_receive_frame(g_codec_ctx, g_frame);
    }

    if (ret >= 0) {
        // Convert to RGB
        _ = c.sws_scale(
            g_sws_ctx,
            @ptrCast(&g_frame.?.*.data),
            @ptrCast(&g_frame.?.*.linesize),
            0,
            g_video_height,
            @ptrCast(&g_rgb_frame.?.*.data),
            @ptrCast(&g_rgb_frame.?.*.linesize),
        );

        // Update texture with new frame data
        const pitch: c_int = g_rgb_frame.?.*.linesize[0];
        _ = SDL.SDL_UpdateTexture(
            g_texture,
            null,
            g_rgb_frame.?.*.data[0],
            pitch,
        );
    }
}

fn cleanupVideo() void {
    if (g_texture != null) {
        SDL.SDL_DestroyTexture(g_texture);
        g_texture = null;
    }
    if (g_rgb_frame != null) {
        if (g_rgb_frame.?.*.data[0] != null) {
            c.av_free(g_rgb_frame.?.*.data[0]);
        }
        c.av_frame_free(@constCast(&g_rgb_frame));
    }
    if (g_frame != null) {
        c.av_frame_free(@constCast(&g_frame));
    }
    if (g_packet != null) {
        c.av_packet_free(@constCast(&g_packet));
    }
    if (g_sws_ctx != null) {
        c.sws_freeContext(g_sws_ctx);
    }
    if (g_codec_ctx != null) {
        c.avcodec_free_context(@constCast(&g_codec_ctx));
    }
    if (g_fmt_ctx != null) {
        c.avformat_close_input(@constCast(&g_fmt_ctx));
    }
}

fn guiFrame(backend: *SDLBackend) bool {
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
    if (g_video_width > 0) {
        const fps = 1000.0 / @as(f32, @floatFromInt(g_frame_duration_ms));
        const status = if (g_is_playing) "Playing" else if (g_video_finished) "Finished" else "Paused";
        var buf: [256]u8 = undefined;
        const text = std.fmt.bufPrint(&buf, "Resolution: {}x{} | FPS: {d:.1} | Status: {s}\n\n", .{ g_video_width, g_video_height, fps, status }) catch "Error formatting";
        info.addText(text, .{});
    }
    info.deinit();

    // Video display area
    if (g_texture != null) {
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
        const frame_aspect = @as(f32, @floatFromInt(g_video_width)) / @as(f32, @floatFromInt(g_video_height));
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
        _ = SDL.SDL_RenderTexture(backend.renderer, g_texture, null, &dst_rect);
    } else {
        var placeholder = dvui.textLayout(@src(), .{}, .{ .expand = .horizontal, .margin = .{ .y = 20 } });
        placeholder.addText("No video loaded. Place 'test.mp4' in the current directory and restart.", .{});
        placeholder.deinit();
    }

    // Playback controls
    {
        var controls = dvui.box(@src(), .{ .dir = .horizontal }, .{ .expand = .horizontal, .margin = .{ .y = 10 } });
        defer controls.deinit();

        if (g_video_width > 0 and !g_video_finished) {
            const button_label = if (g_is_playing) "Pause" else "Play";
            if (dvui.button(@src(), button_label, .{}, .{})) {
                g_is_playing = !g_is_playing;
                if (g_is_playing) {
                    g_last_frame_time = SDL.SDL_GetTicks();
                }
            }
        }

        if (g_video_finished) {
            if (dvui.button(@src(), "Restart", .{}, .{})) {
                // Seek to beginning (simplified restart)
                _ = c.av_seek_frame(g_fmt_ctx, g_video_stream_idx, 0, c.AVSEEK_FLAG_BACKWARD);
                c.avcodec_flush_buffers(g_codec_ctx);
                g_video_finished = false;
                g_is_playing = true;
                g_last_frame_time = SDL.SDL_GetTicks();
            }
        }
    }

    if (dvui.button(@src(), "Debug Window", .{}, .{ .margin = .{ .y = 10 } })) {
        dvui.toggleDebugWindow();
    }

    // Request continuous rendering when video is playing
    // This tells dvui that we need to keep rendering frames even without user input
    if (g_is_playing and !g_video_finished) {
        dvui.refresh(null, @src(), null);
    }

    // Check for quit events
    for (dvui.events()) |*e| {
        if (e.evt == .window and e.evt.window.action == .close) return false;
        if (e.evt == .app and e.evt.app.action == .quit) return false;
    }

    return true;
}
