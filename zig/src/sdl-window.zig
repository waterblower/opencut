const std = @import("std");
const c = @cImport({
    @cInclude("SDL3/SDL.h");
});

pub fn main() !void {
    // Initialize SDL
    if (!c.SDL_Init(c.SDL_INIT_VIDEO)) {
        std.debug.print("SDL could not initialize! SDL_Error: {s}\n", .{c.SDL_GetError()});
        return error.SDLInitializationFailed;
    }
    defer c.SDL_Quit();

    // Create window
    const window = c.SDL_CreateWindow(
        "SDL3 Window Demo",
        800,
        600,
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

    std.debug.print("SDL3 window created successfully!\n", .{});
    std.debug.print("Press ESC or close the window to quit.\n", .{});

    // Main loop
    var running = true;
    var event: c.SDL_Event = undefined;

    while (running) {
        // Handle events
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
                else => {},
            }
        }

        // Clear screen with a nice blue color
        _ = c.SDL_SetRenderDrawColor(renderer, 30, 144, 255, 255);
        _ = c.SDL_RenderClear(renderer);

        // Present renderer
        _ = c.SDL_RenderPresent(renderer);

        // Small delay to reduce CPU usage
        c.SDL_Delay(16); // ~60 FPS
    }

    std.debug.print("Window closed. Goodbye!\n", .{});
}
