const std = @import("std");
const dvui = @import("dvui");

// Change return type: "I might fail (!), and if I succeed, I might return a path or null (?)"
pub fn dialogs(_: std.mem.Allocator) !?[]const u8 {
    var hbox = dvui.box(@src(), .{ .dir = .horizontal }, .{});
    defer hbox.deinit();

    if (dvui.button(@src(), "Open File", .{}, .{})) {
        // 1. Check constraints
        if (dvui.backend.kind == .web) return error.NotImplemented;
        if (!dvui.useTinyFileDialogs) return error.TinyFileDisabled;

        // 2. Perform Action (Fixing File vs Folder mismatch)
        // We use catch to propagate REAL errors (like OutOfMemory), but we handle the logical flow.
        const result = dvui.dialogNativeFileOpen(
            dvui.currentWindow().arena(),
            .{ .title = "Select File" },
        ) catch |err| return err;

        // 3. Return the result (which is already ?[:0]const u8 from dvui)
        return result;
    }

    // 4. Default: No action taken this frame
    return null;
}
