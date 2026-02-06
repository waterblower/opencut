const std = @import("std");

// Pass an allocator so you can manage the memory
pub fn read_folder(allocator: std.mem.Allocator, path: []const u8) !std.ArrayList([]const u8) {
    var dir = try std.fs.cwd().openDir(path, .{ .iterate = true });
    defer dir.close(); // Correct here: we finish using 'dir' before we return

    var list = std.ArrayList([]const u8){};
    // If we fail midway, clean up what we've allocated so far
    errdefer {
        for (list.items) |name| allocator.free(name);
        list.deinit(allocator);
    }

    var iter = dir.iterate();
    while (try iter.next()) |entry| {
        // IMPORTANT: entry.name is temporary! We must make a copy.
        const name_copy = try allocator.dupe(u8, entry.name);
        try list.append(allocator, name_copy);
    }

    return list;
}
