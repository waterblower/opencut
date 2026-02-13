const std = @import("std");

pub fn build(b: *std.Build) void {
    // 1. Common Build Options
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const assets = b.addOptions();
    assets.addOption([]const u8, "app_icon", @embedFile("assets/icon.jpeg"));
    assets.addOption([]const u8, "chinese_font", @embedFile("assets//中文.otf"));
    const assets_module = assets.createModule();

    const sdl_mod = b.createModule(.{
        .root_source_file = b.path("src/sdl3.zig"),
        .target = target,
    });
    sdl_mod.link_libc = true;
    sdl_mod.linkSystemLibrary("SDL3", .{});

    const vid_mod = b.createModule(.{
        .root_source_file = b.path("src/vid.zig"),
        .target = target,
    });
    vid_mod.linkSystemLibrary("avformat", .{});
    vid_mod.linkSystemLibrary("avcodec", .{});
    vid_mod.linkSystemLibrary("avutil", .{});
    vid_mod.linkSystemLibrary("swscale", .{});

    // main program
    setup_opencut(b, target, optimize, assets_module, sdl_mod);

    // attempts
    setupGrayscale(b, target, optimize);
    setup3Panels(b, target, optimize, assets_module);
    setupFileTree(b, target, optimize, assets_module);
    setupPlayAudio(b, target, optimize);
    setupMultiWindow(b, target, optimize, sdl_mod);
}

// ============================================================
// Opencut (Main App)
// ============================================================
fn setup_opencut(
    b: *std.Build,
    target: std.Build.ResolvedTarget,
    optimize: std.builtin.OptimizeMode,
    assets: *std.Build.Module,
    sdl_mod: *std.Build.Module,
) void {
    const name = "opencut";
    const exe = b.addExecutable(.{
        .name = name,
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/opencut.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });

    // Imports & Linking
    exe.root_module.addImport("assets", assets);
    exe.root_module.addImport("sdl", sdl_mod);
    exe.linkLibC();
    exe.linkSystemLibrary("SDL3");
    linkFFmpeg(exe);

    // Build Step: zig build opencut
    const build_step = b.step(name, "Build Open Cut");
    // NOTE: using addInstallArtifact here (instead of b.installArtifact) prevents
    // it from being built by default when running "zig build"
    build_step.dependOn(&b.addInstallArtifact(exe, .{}).step);

    // Run Step: zig build run-opencut
    var run_step = b.step("run-" ++ name, "Run Opencut");
    var run_cmd = b.addRunArtifact(exe);
    if (b.args) |args| run_cmd.addArgs(args);
    run_step.dependOn(&run_cmd.step);

    const tests = b.addTest(.{
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/vid.test.zig"), // 你的测试文件路径
            .target = target,
            .optimize = optimize,
        }),
    });
    linkFFmpeg(tests);
    // Create a "Run" artifact so the build system executes the compiled tests
    const run_tests = b.addRunArtifact(tests);

    // Make the "zig build test" command depend on the RUN step, not just the compile step
    const test_step = b.step("test", "Run tests");
    test_step.dependOn(&run_tests.step);
}

// ============================================================
// Attempts
// ============================================================
fn setupGrayscale(b: *std.Build, target: std.Build.ResolvedTarget, optimize: std.builtin.OptimizeMode) void {
    const exe = b.addExecutable(.{
        .name = "grayscale",
        .root_module = b.createModule(.{
            .root_source_file = b.path("attempts/grayscale.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });

    exe.linkLibC();
    linkFFmpeg(exe);

    const build_step = b.step("grayscale", "Build grayscale converter");
    build_step.dependOn(&b.addInstallArtifact(exe, .{}).step);

    const run_step = b.step("run-grayscale", "Run grayscale converter");
    const run_cmd = b.addRunArtifact(exe);
    if (b.args) |args| run_cmd.addArgs(args);
    run_step.dependOn(&run_cmd.step);
}

// ============================================================
// 4. 3-Panels Example
// ============================================================
fn setup3Panels(b: *std.Build, target: std.Build.ResolvedTarget, optimize: std.builtin.OptimizeMode, assets: *std.Build.Module) void {
    const dvui_dep = b.lazyDependency("dvui", .{
        .target = target,
        .optimize = optimize,
        .backend = .sdl3,
    }) orelse return;

    const exe = b.addExecutable(.{
        .name = "3-panels",
        .root_module = b.createModule(.{
            .root_source_file = b.path("attempts/3-panels.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });

    exe.root_module.addImport("dvui", dvui_dep.module("dvui_sdl3"));
    exe.root_module.addImport("SDLBackend", dvui_dep.module("sdl3"));
    exe.root_module.addImport("assets", assets);
    exe.linkLibC();
    exe.linkSystemLibrary("SDL3");

    const build_step = b.step("3-panels", "Build 3-panel example");
    build_step.dependOn(&b.addInstallArtifact(exe, .{}).step);

    const run_step = b.step("run-3-panels", "Run 3-panel example");
    const run_cmd = b.addRunArtifact(exe);
    if (b.args) |args| run_cmd.addArgs(args);
    run_step.dependOn(&run_cmd.step);
}

// ============================================================
// 5. File Tree Example
// ============================================================
fn setupFileTree(b: *std.Build, target: std.Build.ResolvedTarget, optimize: std.builtin.OptimizeMode, assets: *std.Build.Module) void {
    // Note: We request dvui again here. Zig's build system deduplicates this automatically.
    const dvui_dep = b.lazyDependency("dvui", .{
        .target = target,
        .optimize = optimize,
        .backend = .sdl3,
    }) orelse return;

    const exe = b.addExecutable(.{
        .name = "file-tree",
        .root_module = b.createModule(.{
            .root_source_file = b.path("attempts/file-tree.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });

    exe.root_module.addImport("dvui", dvui_dep.module("dvui_sdl3"));
    exe.root_module.addImport("SDLBackend", dvui_dep.module("sdl3"));
    exe.root_module.addImport("assets", assets);
    exe.linkLibC();
    exe.linkSystemLibrary("SDL3");

    const build_step = b.step("file-tree", "Build file-tree example");
    build_step.dependOn(&b.addInstallArtifact(exe, .{}).step);

    const run_step = b.step("run-file-tree", "Run file-tree example");
    const run_cmd = b.addRunArtifact(exe);
    if (b.args) |args| run_cmd.addArgs(args);
    run_step.dependOn(&run_cmd.step);
}

// ============================================================
// 6. Audio Player
// ============================================================
fn setupPlayAudio(b: *std.Build, target: std.Build.ResolvedTarget, optimize: std.builtin.OptimizeMode) void {
    const zaudio_dep = b.lazyDependency("zaudio", .{
        .target = target,
        .optimize = optimize,
    }) orelse return;

    const audio_options = b.addOptions();
    audio_options.addOption([]const u8, "default_audio_data", @embedFile("test-videos/test.mp3"));

    const exe = b.addExecutable(.{
        .name = "play-audio",
        .root_module = b.createModule(.{
            .root_source_file = b.path("attempts/play-audio.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });

    exe.root_module.addImport("zaudio", zaudio_dep.module("root"));
    exe.root_module.addImport("default_audio", audio_options.createModule());
    exe.linkLibrary(zaudio_dep.artifact("miniaudio"));

    const build_step = b.step("play-audio", "Build audio player");
    build_step.dependOn(&b.addInstallArtifact(exe, .{}).step);

    const run_step = b.step("run-play-audio", "Run audio player");
    const run_cmd = b.addRunArtifact(exe);
    if (b.args) |args| run_cmd.addArgs(args);
    run_step.dependOn(&run_cmd.step);
}

// ============================================================
// 7. Multi-Window Example
// ============================================================
fn setupMultiWindow(b: *std.Build, target: std.Build.ResolvedTarget, optimize: std.builtin.OptimizeMode, sdl_mod: *std.Build.Module) void {
    const exe = b.addExecutable(.{
        .name = "multi-window",
        .root_module = b.createModule(.{
            .root_source_file = b.path("attempts/multi-window.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });

    exe.linkLibC();
    exe.linkSystemLibrary("SDL3");

    exe.root_module.addImport("sdl", sdl_mod);

    const build_step = b.step("multi-window", "Build multi-window example");
    build_step.dependOn(&b.addInstallArtifact(exe, .{}).step);

    const run_step = b.step("run-multi-window", "Run multi-window example");
    const run_cmd = b.addRunArtifact(exe);
    if (b.args) |args| run_cmd.addArgs(args);
    run_step.dependOn(&run_cmd.step);
}

// ============================================================
// Helper: FFmpeg Linking (Don't repeat yourself)
// ============================================================
fn linkFFmpeg(exe: *std.Build.Step.Compile) void {
    const target = exe.root_module.resolved_target.?.result;

    if (target.os.tag == .windows) {
        const ffmpeg_path = "C:\\Program Files\\ffmpeg"; // Adjust to your path
        exe.addLibraryPath(.{ .cwd_relative = ffmpeg_path ++ "\\lib" });
        exe.addIncludePath(.{ .cwd_relative = ffmpeg_path ++ "\\include" });
    }

    exe.linkSystemLibrary("avformat");
    exe.linkSystemLibrary("avcodec");
    exe.linkSystemLibrary("avutil");
    exe.linkSystemLibrary("swscale");
}
