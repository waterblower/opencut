const vid = @import("./vid.zig");
const std = @import("std");
const t = std.testing;

test "split_half" {
    const allocator = t.allocator;
    try vid.split_half(
        allocator,
        "test-videos/test2.mp4",
        "test-videos/test2-1.mp4",
        "test-videos/test2-2.mp4",
    );
}
