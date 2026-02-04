const std = @import("std");
const c = @cImport({
    @cInclude("SDL3/SDL.h");
});
const sdl = @import("sdl");

pub fn main() !void {
    // if (c.SDL_Init(c.SDL_INIT_VIDEO) == false) return error.SDLInitFailed;
    try sdl.Init(c.SDL_INIT_VIDEO);
    defer c.SDL_Quit();

    // --- 创建窗口 A (主界面) ---
    var winA: ?*c.SDL_Window = null;
    var renA: ?*c.SDL_Renderer = null;
    _ = c.SDL_CreateWindowAndRenderer("主窗口", 800, 600, 0, &winA, &renA);

    // --- 创建窗口 B (预览监视器) ---
    var winB: ?*c.SDL_Window = null;
    var renB: ?*c.SDL_Renderer = null;
    _ = c.SDL_CreateWindowAndRenderer("预览窗口", 400, 300, 0, &winB, &renB);

    // 获取它们的 ID，方便后面认人
    const idA = c.SDL_GetWindowID(winA);
    const idB = c.SDL_GetWindowID(winB);

    var running = true;
    var event: c.SDL_Event = undefined;

    // --- 两个窗口共用一个循环 ---
    while (running) {
        if (c.SDL_PollEvent(&event)) {
            switch (event.type) {
                c.SDL_EVENT_QUIT => running = false, // 强制退出整个程序

                // 处理窗口关闭事件
                c.SDL_EVENT_WINDOW_CLOSE_REQUESTED => {
                    const targetID = event.window.windowID;

                    if (targetID == idA) {
                        std.debug.print("主窗口被关闭了，程序退出！\n", .{});
                        running = false;
                    } else if (targetID == idB) {
                        std.debug.print("预览窗口被关闭了，隐藏它！\n", .{});
                        // 这里我们选择隐藏而不是销毁，方便下次再打开
                        _ = c.SDL_HideWindow(winB);
                    }
                },
                else => {},
            }
        }

        // --- 渲染窗口 A (画红色) ---
        _ = c.SDL_SetRenderDrawColor(renA, 100, 0, 0, 255);
        _ = c.SDL_RenderClear(renA);
        _ = c.SDL_RenderPresent(renA);

        // --- 渲染窗口 B (画蓝色) ---
        // 只有当窗口 B 可见时才渲染
        if ((c.SDL_GetWindowFlags(winB) & c.SDL_WINDOW_HIDDEN) == 0) {
            _ = c.SDL_SetRenderDrawColor(renB, 0, 0, 100, 255);
            _ = c.SDL_RenderClear(renB);
            _ = c.SDL_RenderPresent(renB);
        }
    }
}
