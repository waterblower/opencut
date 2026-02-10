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
    defer {
        video.deinit();
    }

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
    const winA = result.window;
    const renA = result.renderer;

    // 1. Create a Texture (Streaming access is for video/frequent updates)
    const texture = c.SDL_CreateTexture(
        renA,
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

    var running = true;

    // 2. 预分配 8-bit 转换缓冲区 (为了避免每帧 malloc，我们在循环外分配)
    // Y Plane: full size
    const y_buf_size = @as(usize, @intCast(width * height));
    const y_buffer = try gpa.alloc(u8, y_buf_size);
    defer gpa.free(y_buffer);

    // U/V Plane: quarter size (width/2 * height/2)
    const uv_buf_size = @as(usize, @intCast((width / 2) * (height / 2)));
    const u_buffer = try gpa.alloc(u8, uv_buf_size);
    defer gpa.free(u_buffer);

    const v_buffer = try gpa.alloc(u8, uv_buf_size);
    defer gpa.free(v_buffer);

    // Main Loop
    var event: c.SDL_Event = undefined;
    while (running) {
        const t0 = now();

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

        // 读取帧并转换
        const f = try video.read_next_frame();
        // const depth = get_bit_depth(@ptrCast(f));
        // print("depth: {d}\n", .{depth});

        if (f) |frame| {

            // Upload YUV data directly to texture
            // For YUV420p: Y plane is full size, U and V are half size
            const result_code = c.SDL_UpdateYUVTexture(
                texture,
                null, // Update entire texture
                frame.data[0], //       Y plane
                frame.linesize[0], //   Y stride
                frame.data[1], //       U plane
                frame.linesize[1], //   U stride
                frame.data[2], //       V plane
                frame.linesize[2], //   V stride
            );

            if (result_code) {
                print("SDL_UpdateYUVTexture failed: {s}\n", .{c.SDL_GetError()});
            }
        }

        // --- 3. Render Texture (Draw it to the screen)
        const rect = compute_rect(winA.?, video);

        _ = c.SDL_RenderTexture(renA, texture, null, &rect);

        _ = c.SDL_RenderPresent(renA);

        // loop time
        const loop_time = now() - t0;
        print("loop time: {d}\n", .{loop_time});

        if (loop_time < video.frameDurationMs()) {
            const sleep_duration = video.frameDurationMs() - @as(u64, @intCast(loop_time));
            print("sleep_duration: {d}\n", .{sleep_duration});
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

fn get_bit_depth(frame: *ffmpeg.AVFrame) i32 {
    // 1. 获取描述符
    const desc = ffmpeg.av_pix_fmt_desc_get(@intCast(frame.format));

    if (desc == null) return 8; // 默认防崩

    // 2. 读取第一个分量 (Component 0, 即 Y/Luma) 的深度
    // desc.comp 是一个数组，存储了 RGBA 或 YUVA 各个分量的信息
    return desc.*.comp[0].depth;
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
