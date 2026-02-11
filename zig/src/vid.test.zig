const vid = @import("./vid.zig");
const std = @import("std");
const t = std.testing;

test "split_half" {
    try vid.split_half(
        "test-videos/test2.mp4",
        "test-videos/test2-1.mp4",
        "test-videos/test2-2.mp4",
    );
}
