const std = @import("std");
const dvui = @import("dvui");
const file = @import("file.zig");

pub fn dialogs(_: std.mem.Allocator) error{ NotImplemented, TinyFileDisabled, OutOfMemory, None }!([:0]const u8) {
    var hbox = dvui.box(@src(), .{ .dir = .horizontal }, .{});
    defer hbox.deinit();

    if (dvui.button(@src(), "Open Folder", .{}, .{})) {
        if (dvui.backend.kind == .web) {
            return error.NotImplemented;
        } else if (!dvui.useTinyFileDialogs) {
            return error.TinyFileDisabled;
        } else {
            const filename = dvui.dialogNativeFileOpen(dvui.currentWindow().arena(), .{ .title = "dvui native folder select" }) catch |err| {
                return err;
            };
            if (filename == null) {
                return error.None;
            }
            return filename.?;
        }
    }
    return error.None;
}
