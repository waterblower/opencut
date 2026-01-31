const std = @import("std");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();
    
    var list = std.ArrayList(u8).init(allocator);
    defer list.deinit();
    
    try list.append(42);
    std.debug.print("Item: {}\n", .{list.items[0]});
}
