const std = @import("std");
const c = @cImport({
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
        return error.InitFailed;
    }
}
