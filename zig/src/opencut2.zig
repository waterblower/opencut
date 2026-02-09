const std = @import("std");
const sdl = @import("sdl");
const c = sdl.c;
const print = std.debug.print;
const vid = @import("vid.zig");
const ffmpeg = @cImport({
    @cInclude("libavformat/avformat.h");
    @cInclude("libavcodec/avcodec.h");
    @cInclude("libavutil/avutil.h");
    @cInclude("libavutil/imgutils.h");
    @cInclude("libswscale/swscale.h");
});

pub fn main() !void {
    var gpa_instance = std.heap.GeneralPurposeAllocator(.{}){};
    const gpa = gpa_instance.allocator();

    const video_path = try parse_args(gpa);

    // Initialize video
    var video = try vid.openVideo(gpa, video_path);
    defer {
        video.deinit();
    }

    try sdl.Init(c.SDL_INIT_VIDEO);
    defer c.SDL_Quit();

    const width = 800;
    const height = 600;

    // --- 创建窗口 (主界面) ---
    const result = try sdl.CreateWindowAndRenderer("主窗口", width, height, 0);
    const winA = result.window;
    const renA = result.renderer;

    // 1. Create a Texture (Streaming access is for video/frequent updates)
    // SDL_PIXELFORMAT_ARGB8888 matches common BGRA/RGBA layouts
    const texture = c.SDL_CreateTexture(
        renA,
        c.SDL_PIXELFORMAT_ARGB8888,
        c.SDL_TEXTUREACCESS_STREAMING,
        width,
        height,
    );
    if (texture == null) {
        print("Failed to create texture: {s}\n", .{c.SDL_GetError()});
        return error.SDL_CreateTexture_Failed;
    }
    defer c.SDL_DestroyTexture(texture);

    // (Optional) Create dummy pixels to verify it works (Red color in ARGB)
    // In real use, this data comes from FFmpeg
    const raw_pixels = try std.heap.page_allocator.alloc(u32, width * height);
    // Fill with a dummy color (e.g., Red)
    @memset(raw_pixels, 0xFFFF0000); // 0xAARRGGBB format
    defer std.heap.page_allocator.free(raw_pixels);

    var running = true;

    // Main Loop
    var event: c.SDL_Event = undefined;
    while (running) {
        std.Thread.sleep(16 * std.time.ns_per_ms);

        if (c.SDL_PollEvent(&event)) {
            switch (event.type) {
                c.SDL_EVENT_QUIT => running = false, // 强制退出整个程序

                // 处理窗口关闭事件
                c.SDL_EVENT_WINDOW_CLOSE_REQUESTED => {
                    const targetID = event.window.windowID;

                    if (targetID == c.SDL_GetWindowID(winA)) {
                        print("主窗口被关闭了，程序退出！\n", .{});
                        running = false;
                    }
                },
                else => {},
            }
        }

        // --- 1. 渲染窗口 (白色) ---
        // _ = c.SDL_SetRenderDrawColor(renA, 255, 255, 255, 255);
        _ = c.SDL_RenderClear(renA);

        // --- 2. Update Texture (Upload pixels to GPU)
        // In your video player, 'raw_pixels.ptr' will be 'video.frame.data[0]'

        const frame = try video.read_next_frame();
        defer ffmpeg.av_frame_free(&frame);

        // const pixel = video.rgb_frame.*.data[0];
        // const pitch = video.rgb_frame.*.linesize[0];
        // _ = c.SDL_UpdateTexture(texture, null, pixel, pitch);

        // --- 3. Render Texture (Draw it to the screen)
        // Passing 'null' for rects means "Draw entire texture to entire window"
        _ = c.SDL_RenderTexture(renA, texture, null, null);

        _ = c.SDL_RenderPresent(renA);
    }
}

fn parse_args(a: std.mem.Allocator) ![]const u8 {
    // Parse command line arguments
    const args = try std.process.argsAlloc(a);
    defer std.process.argsFree(a, args);

    for (args) |arg| {
        std.debug.print("Arg: {s}\n", .{arg});
    }

    const video_path = if (args.len > 1)
        args[1]
    else
        "test-videos/4k.mov";
    return video_path;
}
