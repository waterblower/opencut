const std = @import("std");
pub const c = @cImport({
    @cInclude("SDL3/SDL.h");
});

/// Get the last SDL error message as a Zig string slice
pub fn getError() []const u8 {
    const err = c.SDL_GetError();
    if (err == null) return "";
    return std.mem.span(err);
}

pub fn Init(flags: c.SDL_InitFlags) !void {
    if (!c.SDL_Init(flags)) {
        return error.Failed;
    }
}

// _ = c.SDL_CreateWindowAndRenderer("主窗口", 800, 600, 0, &winA, &renA);
pub fn CreateWindowAndRenderer(
    title: [:0]const u8,
    width: i32,
    height: i32,
    flags: c.SDL_WindowFlags,
) !struct { window: *c.SDL_Window, renderer: *c.SDL_Renderer } {
    var winA: ?*c.SDL_Window = null;
    var renA: ?*c.SDL_Renderer = null;
    if (!c.SDL_CreateWindowAndRenderer(title, width, height, flags, &winA, &renA)) {
        return error.Failed;
    }
    return .{ .window = winA.?, .renderer = renA.? };
}
