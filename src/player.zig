const std = @import("std");
const c = @cImport({
    @cInclude("SDL3/SDL.h");
    @cInclude("libavformat/avformat.h");
    @cInclude("libavcodec/avcodec.h");
    @cInclude("libavutil/avutil.h");
    @cInclude("libavutil/imgutils.h");
    @cInclude("libswscale/swscale.h");
});

// Global state for event watch callback
var g_renderer: ?*c.SDL_Renderer = null;
var g_texture: ?*c.SDL_Texture = null;
var g_video_width: c_int = 0;
var g_video_height: c_int = 0;

// Event watch callback that runs even during blocking resize operations
fn eventWatchCallback(userdata: ?*anyopaque, event: [*c]c.SDL_Event) callconv(.c) bool {
    _ = userdata;

    // Check for window events during resize
    if (event.*.type == c.SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED or
        event.*.type == c.SDL_EVENT_WINDOW_RESIZED or
        event.*.type == c.SDL_EVENT_WINDOW_EXPOSED)
    {
        if (g_renderer != null and g_texture != null) {
            // Get the window from the event
            const window_id = event.*.window.windowID;
            const window = c.SDL_GetWindowFromID(window_id);

            if (window != null) {
                // Get current window size
                var window_width: c_int = 0;
                var window_height: c_int = 0;
                _ = c.SDL_GetWindowSize(window, &window_width, &window_height);

                // Calculate aspect ratio preserving destination rectangle
                const frame_aspect = @as(f32, @floatFromInt(g_video_width)) / @as(f32, @floatFromInt(g_video_height));
                const window_aspect = @as(f32, @floatFromInt(window_width)) / @as(f32, @floatFromInt(window_height));

                var dst_rect: c.SDL_FRect = undefined;

                if (window_aspect > frame_aspect) {
                    // Window is wider than frame - fit to height
                    dst_rect.h = @floatFromInt(window_height);
                    dst_rect.w = dst_rect.h * frame_aspect;
                    dst_rect.x = (@as(f32, @floatFromInt(window_width)) - dst_rect.w) / 2.0;
                    dst_rect.y = 0;
                } else {
                    // Window is taller than frame - fit to width
                    dst_rect.w = @floatFromInt(window_width);
                    dst_rect.h = dst_rect.w / frame_aspect;
                    dst_rect.x = 0;
                    dst_rect.y = (@as(f32, @floatFromInt(window_height)) - dst_rect.h) / 2.0;
                }

                // Clear screen
                _ = c.SDL_SetRenderDrawColor(g_renderer, 0, 0, 0, 255);
                _ = c.SDL_RenderClear(g_renderer);

                // Render texture with aspect ratio preserved
                _ = c.SDL_RenderTexture(g_renderer, g_texture, null, &dst_rect);

                // Present renderer
                _ = c.SDL_RenderPresent(g_renderer);
            }
        }
    }

    return true;
}

pub fn main() !void {
    // Initialize SDL
    if (!c.SDL_Init(c.SDL_INIT_VIDEO)) {
        std.debug.print("SDL could not initialize! SDL_Error: {s}\n", .{c.SDL_GetError()});
        return error.SDLInitializationFailed;
    }
    defer c.SDL_Quit();

    // Open video file
    var fmt_ctx: ?*c.AVFormatContext = null;
    if (c.avformat_open_input(&fmt_ctx, "test.mp4", null, null) < 0) {
        std.debug.print("Could not open video file\n", .{});
        return error.CouldNotOpenFile;
    }
    defer c.avformat_close_input(&fmt_ctx);

    // Retrieve stream information
    if (c.avformat_find_stream_info(fmt_ctx, null) < 0) {
        std.debug.print("Could not find stream information\n", .{});
        return error.CouldNotFindStreamInfo;
    }

    // Find the first video stream
    var video_stream_idx: c_int = -1;
    var i: c_uint = 0;
    while (i < fmt_ctx.?.nb_streams) : (i += 1) {
        if (fmt_ctx.?.streams[i].*.codecpar.*.codec_type == c.AVMEDIA_TYPE_VIDEO) {
            video_stream_idx = @intCast(i);
            break;
        }
    }

    if (video_stream_idx == -1) {
        std.debug.print("Could not find video stream\n", .{});
        return error.NoVideoStream;
    }

    // Get codec parameters
    const codecpar = fmt_ctx.?.streams[@intCast(video_stream_idx)].*.codecpar;

    // Find decoder
    const codec = c.avcodec_find_decoder(codecpar.*.codec_id);
    if (codec == null) {
        std.debug.print("Unsupported codec\n", .{});
        return error.UnsupportedCodec;
    }

    // Allocate codec context
    const codec_ctx = c.avcodec_alloc_context3(codec);
    if (codec_ctx == null) {
        std.debug.print("Could not allocate codec context\n", .{});
        return error.CouldNotAllocateCodecContext;
    }
    defer c.avcodec_free_context(@constCast(&codec_ctx));

    // Copy codec parameters to context
    if (c.avcodec_parameters_to_context(codec_ctx, codecpar) < 0) {
        std.debug.print("Could not copy codec parameters\n", .{});
        return error.CouldNotCopyCodecParams;
    }

    // Open codec
    if (c.avcodec_open2(codec_ctx, codec, null) < 0) {
        std.debug.print("Could not open codec\n", .{});
        return error.CouldNotOpenCodec;
    }

    const width = codec_ctx.*.width;
    const height = codec_ctx.*.height;

    std.debug.print("Video: {}x{}\n", .{ width, height });

    // Allocate frame
    const frame = c.av_frame_alloc();
    if (frame == null) {
        std.debug.print("Could not allocate frame\n", .{});
        return error.CouldNotAllocateFrame;
    }
    defer c.av_frame_free(@constCast(&frame));

    // Allocate RGB frame
    const rgb_frame = c.av_frame_alloc();
    if (rgb_frame == null) {
        std.debug.print("Could not allocate RGB frame\n", .{});
        return error.CouldNotAllocateRGBFrame;
    }
    defer c.av_frame_free(@constCast(&rgb_frame));

    // Allocate packet
    const packet = c.av_packet_alloc();
    if (packet == null) {
        std.debug.print("Could not allocate packet\n", .{});
        return error.CouldNotAllocatePacket;
    }
    defer c.av_packet_free(@constCast(&packet));

    // Initialize SWS context for color conversion
    const sws_ctx = c.sws_getContext(
        width,
        height,
        codec_ctx.*.pix_fmt,
        width,
        height,
        c.AV_PIX_FMT_RGB24,
        c.SWS_BILINEAR,
        null,
        null,
        null,
    );
    if (sws_ctx == null) {
        std.debug.print("Could not initialize SWS context\n", .{});
        return error.CouldNotInitSWSContext;
    }
    defer c.sws_freeContext(sws_ctx);

    // Allocate buffer for RGB frame
    const num_bytes = c.av_image_get_buffer_size(c.AV_PIX_FMT_RGB24, width, height, 1);
    const buffer = c.av_malloc(@intCast(num_bytes));
    if (buffer == null) {
        std.debug.print("Could not allocate buffer\n", .{});
        return error.CouldNotAllocateBuffer;
    }
    defer c.av_free(buffer);

    _ = c.av_image_fill_arrays(
        @ptrCast(&rgb_frame.*.data),
        @ptrCast(&rgb_frame.*.linesize),
        @ptrCast(buffer),
        c.AV_PIX_FMT_RGB24,
        width,
        height,
        1,
    );

    // Read frames until we get the first video frame
    var frame_decoded = false;
    while (c.av_read_frame(fmt_ctx, packet) >= 0) {
        if (packet.*.stream_index == video_stream_idx) {
            // Send packet to decoder
            if (c.avcodec_send_packet(codec_ctx, packet) < 0) {
                std.debug.print("Error sending packet to decoder\n", .{});
                c.av_packet_unref(packet);
                continue;
            }

            // Receive frame from decoder
            while (c.avcodec_receive_frame(codec_ctx, frame) >= 0) {
                // Convert to RGB
                _ = c.sws_scale(
                    sws_ctx,
                    @ptrCast(&frame.*.data),
                    @ptrCast(&frame.*.linesize),
                    0,
                    height,
                    @ptrCast(&rgb_frame.*.data),
                    @ptrCast(&rgb_frame.*.linesize),
                );

                frame_decoded = true;
                break;
            }

            if (frame_decoded) {
                c.av_packet_unref(packet);
                break;
            }
        }
        c.av_packet_unref(packet);
    }

    if (!frame_decoded) {
        std.debug.print("Could not decode first frame\n", .{});
        return error.CouldNotDecodeFrame;
    }

    std.debug.print("First frame decoded successfully!\n", .{});

    // Create SDL window
    const window = c.SDL_CreateWindow(
        "Video Player - First Frame",
        width,
        height,
        c.SDL_WINDOW_RESIZABLE,
    );
    if (window == null) {
        std.debug.print("Window could not be created! SDL_Error: {s}\n", .{c.SDL_GetError()});
        return error.WindowCreationFailed;
    }
    defer c.SDL_DestroyWindow(window);

    // Create renderer
    const renderer = c.SDL_CreateRenderer(window, null);
    if (renderer == null) {
        std.debug.print("Renderer could not be created! SDL_Error: {s}\n", .{c.SDL_GetError()});
        return error.RendererCreationFailed;
    }
    defer c.SDL_DestroyRenderer(renderer);

    // Store global references for event watch callback
    g_renderer = renderer;
    g_video_width = width;
    g_video_height = height;

    // Register event watch to handle rendering during resize
    _ = c.SDL_AddEventWatch(eventWatchCallback, null);
    defer c.SDL_RemoveEventWatch(eventWatchCallback, null);

    // Create texture
    const texture = c.SDL_CreateTexture(
        renderer,
        c.SDL_PIXELFORMAT_RGB24,
        c.SDL_TEXTUREACCESS_STREAMING,
        width,
        height,
    );
    if (texture == null) {
        std.debug.print("Texture could not be created! SDL_Error: {s}\n", .{c.SDL_GetError()});
        return error.TextureCreationFailed;
    }
    defer c.SDL_DestroyTexture(texture);

    // Store texture reference for event watch callback
    g_texture = texture;

    // Update texture with RGB data
    const pitch: c_int = rgb_frame.*.linesize[0];
    _ = c.SDL_UpdateTexture(
        texture,
        null,
        rgb_frame.*.data[0],
        pitch,
    );

    std.debug.print("Press ESC or close the window to quit.\n", .{});

    // Main loop
    var running = true;
    var event: c.SDL_Event = undefined;

    while (running) {
        // Handle events (non-blocking)
        while (c.SDL_PollEvent(&event)) {
            switch (event.type) {
                c.SDL_EVENT_QUIT => {
                    running = false;
                },
                c.SDL_EVENT_KEY_DOWN => {
                    if (event.key.key == c.SDLK_ESCAPE) {
                        running = false;
                    }
                },
                c.SDL_EVENT_WINDOW_RESIZED, c.SDL_EVENT_WINDOW_EXPOSED => {
                    // These events are handled by the event watch callback
                    // which runs even during blocking resize operations
                },
                else => {},
            }
        }

        // Get current window size
        var window_width: c_int = 0;
        var window_height: c_int = 0;
        _ = c.SDL_GetWindowSize(window, &window_width, &window_height);

        // Calculate aspect ratio preserving destination rectangle
        const frame_aspect = @as(f32, @floatFromInt(width)) / @as(f32, @floatFromInt(height));
        const window_aspect = @as(f32, @floatFromInt(window_width)) / @as(f32, @floatFromInt(window_height));

        var dst_rect: c.SDL_FRect = undefined;

        if (window_aspect > frame_aspect) {
            // Window is wider than frame - fit to height
            dst_rect.h = @floatFromInt(window_height);
            dst_rect.w = dst_rect.h * frame_aspect;
            dst_rect.x = (@as(f32, @floatFromInt(window_width)) - dst_rect.w) / 2.0;
            dst_rect.y = 0;
        } else {
            // Window is taller than frame - fit to width
            dst_rect.w = @floatFromInt(window_width);
            dst_rect.h = dst_rect.w / frame_aspect;
            dst_rect.x = 0;
            dst_rect.y = (@as(f32, @floatFromInt(window_height)) - dst_rect.h) / 2.0;
        }

        // Clear screen
        _ = c.SDL_SetRenderDrawColor(renderer, 0, 0, 0, 255);
        _ = c.SDL_RenderClear(renderer);

        // Render texture with aspect ratio preserved
        _ = c.SDL_RenderTexture(renderer, texture, null, &dst_rect);

        // Present renderer
        _ = c.SDL_RenderPresent(renderer);

        // Small delay to reduce CPU usage
        c.SDL_Delay(16); // ~60 FPS
    }

    std.debug.print("Window closed. Goodbye!\n", .{});
}
