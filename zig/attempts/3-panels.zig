const std = @import("std");
const dvui = @import("dvui");
const SDLBackend = @import("SDLBackend");

// Store split positions as absolute ratios of total width
var left_divider_pos: f32 = 0.25; // First divider at 25% of total width
var right_divider_pos: f32 = 0.65; // Second divider at 65% of total width (middle panel is 40%)

pub fn main() !void {
    var gpa_instance = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa_instance.deinit();
    const gpa = gpa_instance.allocator();

    var backend = try SDLBackend.initWindow(.{
        .allocator = gpa,
        .size = .{ .w = 1200.0, .h = 800.0 },
        .min_size = .{ .w = 800.0, .h = 600.0 },
        .vsync = true,
        .title = "3-Panel IDE Layout Example",
    });
    defer backend.deinit();

    var win = try dvui.Window.init(@src(), gpa, backend.backend(), .{});
    defer win.deinit();

    const interrupted = false;

    main_loop: while (true) {
        const nstime = win.beginWait(interrupted);

        try win.begin(nstime);

        try backend.addAllEvents(&win);

        const quit = try frame();
        if (quit) break :main_loop;

        const end_micros = try win.end(.{});

        try backend.setCursor(win.cursorRequested());
        try backend.textInputRect(win.textInputRequested());

        try backend.renderPresent();

        const wait_event_micros = win.waitTime(end_micros);
        _ = try backend.waitEventTimeout(wait_event_micros);
    }
}

fn frame() !bool {
    var quit = false;

    // Handle window events
    for (dvui.events()) |*e| {
        if (e.evt == .window and e.evt.window.action == .close) quit = true;
        if (e.evt == .app and e.evt.app.action == .quit) quit = true;
    }

    threePanelsDemo();

    return quit;
}

/// Three-panel vertical split layout with draggable dividers - IDE style
/// This demonstrates a typical IDE layout with:
/// - Left panel: File explorer
/// - Middle panel: Code editor
/// - Right panel: Properties/tools
pub fn threePanelsDemo() void {
    var main_box = dvui.box(@src(), .{}, .{
        .expand = .both,
        .background = true,
        .style = .window,
    });
    defer main_box.deinit();

    threePanelLayout();
}

/// Three-panel vertical split layout with draggable dividers
fn threePanelLayout() void {
    const parent_rect = dvui.parentGet().data().contentRect();
    const total_width = parent_rect.w;

    if (total_width <= 0) return;

    // Use an overlay to create absolute-positioned panels
    var overlay = dvui.overlay(@src(), .{ .expand = .both });
    defer overlay.deinit();

    // Clamp divider positions
    left_divider_pos = std.math.clamp(left_divider_pos, 0.05, right_divider_pos - 0.05);
    right_divider_pos = std.math.clamp(right_divider_pos, left_divider_pos + 0.05, 0.95);

    // Calculate panel widths
    const left_width = total_width * left_divider_pos;
    const middle_width = total_width * (right_divider_pos - left_divider_pos);
    const right_width = total_width * (1.0 - right_divider_pos);

    // Left panel
    {
        var panel_box = dvui.box(@src(), .{}, .{
            .min_size_content = .{ .w = left_width, .h = parent_rect.h },
            .max_size_content = .{ .w = left_width, .h = parent_rect.h },
            .background = true,
            .color_fill = .{ .r = 0x20, .g = 0x20, .b = 0x28, .a = 255 },
            .border = dvui.Rect.all(1),
            .id_extra = 1,
        });
        defer panel_box.deinit();
    }

    // Middle panel
    {
        const middle_x = left_width;
        var panel_box = dvui.box(@src(), .{}, .{
            .min_size_content = .{ .w = middle_width, .h = parent_rect.h },
            .max_size_content = .{ .w = middle_width, .h = parent_rect.h },
            .rect = .{ .x = middle_x, .y = 0, .w = middle_width, .h = parent_rect.h },
            .background = true,
            .color_fill = .{ .r = 0x28, .g = 0x28, .b = 0x30, .a = 255 },
            .border = dvui.Rect.all(1),
            .id_extra = 2,
        });
        defer panel_box.deinit();
    }

    // Right panel
    {
        const right_x = total_width * right_divider_pos;
        var panel_box = dvui.box(@src(), .{}, .{
            .min_size_content = .{ .w = right_width, .h = parent_rect.h },
            .max_size_content = .{ .w = right_width, .h = parent_rect.h },
            .rect = .{ .x = right_x, .y = 0, .w = right_width, .h = parent_rect.h },
            .background = true,
            .color_fill = .{ .r = 0x20, .g = 0x20, .b = 0x28, .a = 255 },
            .border = dvui.Rect.all(1),
            .id_extra = 3,
        });
        defer panel_box.deinit();
    }

    // Draw dividers LAST so they're on top and clickable
    // Left divider (draggable)
    {
        const divider_width: f32 = 8;
        const divider_x = left_width - divider_width / 2;

        const dragging_ptr = dvui.dataGetPtrDefault(null, dvui.parentGet().extendId(@src(), 100), "_dragging_left", bool, false);

        var divider: dvui.ButtonWidget = undefined;
        const divider_color = if (dragging_ptr.*)
            dvui.Color{ .r = 0x60, .g = 0x80, .b = 0xFF, .a = 255 } // Blue when dragging
        else
            dvui.Color{ .r = 0x40, .g = 0x40, .b = 0x48, .a = 255 }; // Gray normally

        divider.init(@src(), .{}, .{
            .min_size_content = .{ .w = divider_width, .h = parent_rect.h },
            .rect = .{ .x = divider_x, .y = 0, .w = divider_width, .h = parent_rect.h },
            .background = true,
            .color_fill = divider_color,
            .color_fill_hover = .{ .r = 0x50, .g = 0x50, .b = 0x58, .a = 255 },
            .id_extra = 100,
        });
        defer divider.deinit();

        divider.processEvents();

        // Check for mouse press on divider to start dragging
        if (divider.clicked()) {
            dragging_ptr.* = true;
        }

        // Handle dragging motion and release
        for (dvui.events()) |*e| {
            if (e.evt == .mouse) {
                if (e.evt.mouse.action == .release and e.evt.mouse.button == .left) {
                    if (dragging_ptr.*) {
                        dragging_ptr.* = false;
                    }
                } else if (e.evt.mouse.action == .motion and dragging_ptr.*) {
                    const mouse_x = e.evt.mouse.p.x;
                    left_divider_pos = std.math.clamp(mouse_x / total_width, 0.05, right_divider_pos - 0.05);
                    e.handled = true;
                }
            }
        }

        divider.drawBackground();
    }

    // Right divider (draggable)
    {
        const divider_width: f32 = 8;
        const divider_x = (total_width * right_divider_pos) - divider_width / 2;

        const dragging_ptr = dvui.dataGetPtrDefault(null, dvui.parentGet().extendId(@src(), 200), "_dragging_right", bool, false);

        var divider: dvui.ButtonWidget = undefined;
        const divider_color = if (dragging_ptr.*)
            dvui.Color{ .r = 0x60, .g = 0x80, .b = 0xFF, .a = 255 } // Blue when dragging
        else
            dvui.Color{ .r = 0x40, .g = 0x40, .b = 0x48, .a = 255 }; // Gray normally

        divider.init(@src(), .{}, .{
            .min_size_content = .{ .w = divider_width, .h = parent_rect.h },
            .rect = .{ .x = divider_x, .y = 0, .w = divider_width, .h = parent_rect.h },
            .background = true,
            .color_fill = divider_color,
            .color_fill_hover = .{ .r = 0x50, .g = 0x50, .b = 0x58, .a = 255 },
            .id_extra = 200,
        });
        defer divider.deinit();

        divider.processEvents();

        // Check for mouse press on divider to start dragging
        if (divider.clicked()) {
            dragging_ptr.* = true;
        }

        // Handle dragging motion and release
        for (dvui.events()) |*e| {
            if (e.evt == .mouse) {
                if (e.evt.mouse.action == .release and e.evt.mouse.button == .left) {
                    if (dragging_ptr.*) {
                        dragging_ptr.* = false;
                    }
                } else if (e.evt.mouse.action == .motion and dragging_ptr.*) {
                    const mouse_x = e.evt.mouse.p.x;
                    right_divider_pos = std.math.clamp(mouse_x / total_width, left_divider_pos + 0.05, 0.95);
                    e.handled = true;
                }
            }
        }

        divider.drawBackground();
    }
}

test {
    @import("std").testing.refAllDecls(@This());
}
