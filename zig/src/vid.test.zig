const vid = @import("./vid.zig");
const std = @import("std");
const t = std.testing;

test "split_half" {
    std.debug.print("running test\n", .{});
    // const allocator = t.allocator;
    try vid.split_half_v2(
        "test-videos/test2.mp4",
        "test-videos/test2-1.mp4",
        "test-videos/test2-2.mp4",
    );
}
