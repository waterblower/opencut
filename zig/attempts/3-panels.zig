const std = @import("std");
const dvui = @import("dvui");
const SDLBackend = @import("SDLBackend");
const assets = @import("assets");

pub const dvui_app: dvui.App = .{
    .config = .{
        .options = .{
            .size = .{ .w = 800.0, .h = 600.0 },
            .min_size = .{ .w = 250.0, .h = 350.0 },
            .title = "File Tree",
            .icon = assets.app_icon,
            .window_init_options = .{
                // Could set a default theme here
                // .theme = dvui.Theme.builtin.dracula,

            },
        },
    },
    .frameFn = AppFrame,
    .initFn = AppInit,
    .deinitFn = deinit,
};
fn deinit() void {}
pub const main = dvui.App.main;

pub fn AppFrame() !dvui.App.Result {
    var pane = dvui.paned(
        @src(),
        .{
            .direction = .horizontal,
            .collapsed_size = 600,
        },
        .{
            .expand = .both,

            .background = true,
            .min_size_content = .{ .h = 100 },
            .color_fill = .{ .r = 255, .g = 255, .b = 255, .a = 255 },
        },
    );
    if (pane.showFirst()) {
        var box = dvui.box(
            @src(),
            .{},
            .{
                .expand = .both,
                .background = true,
                .color_border = .{ .r = 0, .g = 0, .b = 0, .a = 255 },
                .border = .rect(0, 0, 1, 0),
            },
        );
        {
            var tl = dvui.textLayout(@src(), .{}, .{});
            tl.addText("这是中文", .{});
            tl.deinit();
        }
        defer box.deinit();
    }
    if (pane.showSecond()) {
        var pane2 = dvui.paned(
            @src(),
            .{
                .direction = .horizontal,
                .collapsed_size = 150,
            },
            .{
                .expand = .both,
                .background = true,
                .min_size_content = .{ .h = 100 },
                .color_fill = .{ .r = 255, .g = 255, .b = 255, .a = 255 },
                .color_border = .{ .r = 0, .g = 0, .b = 0, .a = 255 },
                .border = .rect(1, 0, 0, 0),
            },
        );
        {
            if (pane2.showFirst()) {
                var box = dvui.box(
                    @src(),
                    .{},
                    .{
                        .expand = .both,
                        .background = true,
                        .color_border = .{ .r = 0, .g = 0, .b = 0, .a = 255 },
                        .border = .rect(0, 0, 1, 0),
                    },
                );
                box.deinit();
            }
            if (pane2.showSecond()) {
                var box = dvui.box(
                    @src(),
                    .{},
                    .{
                        .expand = .both,
                        .color_border = .{ .r = 0, .g = 0, .b = 0, .a = 255 },
                        .border = .rect(1, 0, 0, 0),
                    },
                );
                {}
                box.deinit();
            }
        }
        pane2.deinit();
    }
    pane.deinit();
    return .ok;
}

pub fn AppInit(_: *dvui.Window) !void {
    try dvui.addFont("中文", @embedFile("中文.otf"), dvui.currentWindow().gpa);

    // Set the font globally in the theme
    var theme = dvui.themeGet();

    // Find the font in the database
    for (dvui.currentWindow().fonts.database.items) |*dbs| {
        if (std.mem.eql(u8, dbs.familyName(), "中文")) {
            theme.font_body = dbs.font();
            theme.font_heading = dbs.font();
            break;
        }
    }

    theme.font_body.size = 26;

    dvui.themeSet(theme);
}
