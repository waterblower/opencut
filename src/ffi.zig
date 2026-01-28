const std = @import("std");

/// Simple add function that can be called from Flutter via FFI
/// This uses the C calling convention to be compatible with FFI
export fn add(a: i32, b: i32) i32 {
    return a + b;
}
