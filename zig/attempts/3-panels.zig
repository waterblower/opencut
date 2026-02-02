const std = @import("std");
const dvui = @import("dvui");
const SDLBackend = @import("SDLBackend");

pub const dvui_app: dvui.App = .{
    .config = .{
        .options = .{
            .size = .{ .w = 800.0, .h = 600.0 },
            .min_size = .{ .w = 250.0, .h = 350.0 },
            .title = "File Tree",
            .icon = @embedFile("./icon.jpeg"),
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
    var pane = dvui.paned(@src(), .{ .direction = .horizontal, .collapsed_size = 600 }, .{ .expand = .both, .background = false, .min_size_content = .{ .h = 100 } });
    if(pane.showFirst()) {
        var tl = dvui.textLayout(@src(), .{}, .{});
        tl.addText("Here is a textLayout with a bunch of text in it that would overflow the right edge but the dialog has a max_size_content", .{});
        tl.deinit();
    }
    if(pane.showSecond()) {
        var tl = dvui.textLayout(@src(), .{}, .{});
        tl.addText("Here is a textLayout with a bunch of text in it that would overflow the right edge but the dialog has a max_size_content", .{});
        tl.deinit();
    }
    pane.deinit();
    return .ok;
}

pub fn AppInit(_: *dvui.Window) !void {}
