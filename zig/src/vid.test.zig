const vid = @import("./vid.zig");
const std = @import("std");
const t = std.testing;

test "split_half" {
    try vid.split_half(
        "test-videos/test1.mp4",
        "test-videos/test1-split_1.mp4",
        "test-videos/test1-split_2.mp4",
    );
}
