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
const now = std.time.milliTimestamp;

pub fn main() !void {
    var gpa_instance = std.heap.GeneralPurposeAllocator(.{}){};
    const gpa = gpa_instance.allocator();

    const video_path = try parse_args(gpa);

    // Initialize video
    var video = try vid.openVideo(gpa, video_path);
    defer video.deinit();

    try sdl.Init(c.SDL_INIT_VIDEO);
    defer c.SDL_Quit();

    const width = 800;
    const height = 600;

    // --- 创建窗口 (主界面) ---
    const result = try sdl.CreateWindowAndRenderer(
        "主窗口",
        width,
        height,
        c.SDL_WINDOW_RESIZABLE,
    );
    const window = result.window;
    const renderer = result.renderer;

    // 1. Create a Texture (Streaming access is for video/frequent updates)
    const texture = c.SDL_CreateTexture(
        renderer,
        c.SDL_PIXELFORMAT_IYUV, // <--- 关键修改
        c.SDL_TEXTUREACCESS_STREAMING,
        video.width,
        video.height,
    );
    if (texture == null) {
        print("Failed to create texture: {s}\n", .{c.SDL_GetError()});
        return error.SDL_CreateTexture_Failed;
    }
    defer c.SDL_DestroyTexture(texture);

    try UI(renderer, window, texture, video);
}

fn UI(
    renderer: *c.SDL_Renderer,
    window: *c.SDL_Window,
    texture: *c.SDL_Texture,
    video: *vid.Video,
) !void {
    // Main Loop
    var event: c.SDL_Event = undefined;
    while (true) {
        const t0 = now();

        if (c.SDL_PollEvent(&event)) {
            switch (event.type) {
                c.SDL_EVENT_QUIT => {
                    return; // 退出UI
                },
                // 处理窗口关闭事件
                c.SDL_EVENT_WINDOW_CLOSE_REQUESTED => {
                    const targetID = event.window.windowID;

                    if (targetID == c.SDL_GetWindowID(window)) {
                        print("窗口被关闭了，退出UI！\n", .{});
                        return;
                    }
                },
                else => {},
            }
        }

        // --- 1. 渲染窗口 (白色) ---
        _ = c.SDL_RenderClear(renderer);

        // --- 2. Update Texture (Upload pixels to GPU)
        // In your video player, 'raw_pixels.ptr' will be 'video.frame.data[0]'

        // 读取帧并转换
        const f = try video.read_next_frame();
        if (f) |frame| {
            // 1. 获取位深
            // 注意：这里需要 @ptrCast 解决类型冲突
            const depth = try vid.get_bit_depth(@ptrCast(frame));
            if (depth == 10) {
                // --- 路径 A: 10-bit 视频 (需要转换) ---

                const sws_ctx = video.sws_ctx;
                const dst = video.sws_frame;

                _ = ffmpeg.sws_scale(
                    @ptrCast(sws_ctx),
                    &frame.data,
                    &frame.linesize,
                    0,
                    video.height,
                    &dst.data,
                    &dst.linesize,
                );
                // 用转换后的 8bit frame 给 SDL
                const ok = c.SDL_UpdateYUVTexture(
                    texture,
                    null,
                    dst.data[0],
                    dst.linesize[0],
                    dst.data[1],
                    dst.linesize[1],
                    dst.data[2],
                    dst.linesize[2],
                );
                if (!ok) {
                    print("SDL_UpdateYUVTexture failed: {s}\n", .{c.SDL_GetError()});
                    return error.SDL_UpdateYUVTexture_Failed;
                }
            } else if (depth == 8) {
                // --- 路径 B: 8-bit 视频 (直接上传，性能最高) ---
                _ = c.SDL_UpdateYUVTexture(texture, null, frame.data[0], frame.linesize[0], frame.data[1], frame.linesize[1], frame.data[2], frame.linesize[2]);
            } else {
                print("do not support depth {d}\n", .{depth});
                return error.DepthNotSupported;
            }
        }

        // --- 3. Render Texture (Draw it to the screen)
        const rect = compute_rect(window, video);
        _ = c.SDL_RenderTexture(renderer, texture, null, &rect);
        _ = c.SDL_RenderPresent(renderer);

        // loop time
        const loop_time = now() - t0;
        print("\r\x1b[K loop time: {d}ms", .{loop_time});

        if (loop_time < video.frameDurationMs()) {
            const sleep_duration = video.frameDurationMs() - @as(u64, @intCast(loop_time));
            std.Thread.sleep(sleep_duration * 1000 * 1000);
        }
    }
}

fn parse_args(a: std.mem.Allocator) ![]const u8 {
    // Parse command line arguments
    const args = try std.process.argsAlloc(a);

    for (args) |arg| {
        std.debug.print("Arg: {s}\n", .{arg});
    }

    const video_path = if (args.len > 1)
        args[1]
    else
        "test-videos/4k.mov";
    return video_path;
}

fn compute_rect(win: *c.SDL_Window, video: *vid.Video) c.SDL_FRect {
    // 1. 获取当前窗口大小 (用户可能拖拽改变了大小)
    var win_w: i32 = 0;
    var win_h: i32 = 0;
    _ = c.SDL_GetWindowSize(win, &win_w, &win_h);

    // 2. 计算保持比例的矩形 (Letterbox / Aspect Fit)
    const vid_w = @as(f32, @floatFromInt(video.width));
    const vid_h = @as(f32, @floatFromInt(video.height));
    const screen_w = @as(f32, @floatFromInt(win_w));
    const screen_h = @as(f32, @floatFromInt(win_h));

    const scale = @min(screen_w / vid_w, screen_h / vid_h);

    const draw_w = vid_w * scale;
    const draw_h = vid_h * scale;
    const draw_x = (screen_w - draw_w) / 2.0;
    const draw_y = (screen_h - draw_h) / 2.0;

    // 3. 定义目标矩形
    const dst_rect = c.SDL_FRect{
        .x = draw_x,
        .y = draw_y,
        .w = draw_w,
        .h = draw_h,
    };
    return dst_rect;
}
