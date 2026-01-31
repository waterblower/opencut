const std = @import("std");
const dvui = @import("dvui");
const file = @import("file.zig");

pub fn dialogs(a: std.mem.Allocator, demo_win_id: dvui.Id) void {
    var hbox = dvui.box(@src(), .{ .dir = .horizontal }, .{});
    defer hbox.deinit();

    if (dvui.button(@src(), "Open Folder", .{}, .{})) {
        if (dvui.backend.kind == .web) {
            dvui.toast(@src(), .{ .subwindow_id = demo_win_id, .message = "Not implemented for web" });
        } else if (!dvui.useTinyFileDialogs) {
            dvui.toast(@src(), .{ .subwindow_id = demo_win_id, .message = "Tiny File Dilaogs disabled" });
        } else {
            const filename = dvui.dialogNativeFolderSelect(dvui.currentWindow().arena(), .{ .title = "dvui native folder select" }) catch |err| blk: {
                dvui.log.debug("Could not open folder select dialog, got {any}", .{err});
                break :blk null;
            };
            const f = filename.?;

            const files = file.read_folder(a, f) catch |err| {
                dvui.log.debug("Could not read folder, got {any}", .{err});
                dvui.dialog(@src(), .{}, .{ .modal = false, .title = "Folder Select Error", .ok_label = "Done", .message = "Failed to read folder" });
                return;
            };
            for (files.items) |file_name| {
                std.debug.print("File: {s}\n", .{file_name});
            }

            dvui.dialog(@src(), .{}, .{ .modal = false, .title = "Folder Select Result", .ok_label = "Done", .message = f });
        }
    }

    if (dvui.button(@src(), "Save File", .{}, .{})) {
        if (dvui.backend.kind == .web) {
            dvui.dialog(@src(), .{}, .{ .modal = false, .title = "Save File", .ok_label = "Ok", .message = "Not available on the web.  For file download, see \"Save Plot\" in the plots example." });
        } else if (!dvui.useTinyFileDialogs) {
            dvui.toast(@src(), .{ .subwindow_id = demo_win_id, .message = "Tiny File Dilaogs disabled" });
        } else {
            const filename = dvui.dialogNativeFileSave(dvui.currentWindow().arena(), .{ .title = "dvui native file save" }) catch |err| blk: {
                dvui.log.debug("Could not open file save dialog, got {any}", .{err});
                break :blk null;
            };
            if (filename) |f| {
                dvui.dialog(@src(), .{}, .{ .modal = false, .title = "File Save Result", .ok_label = "Done", .message = f });
            }
        }
    }
}
