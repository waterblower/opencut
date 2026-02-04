const std = @import("std");

pub fn build(b: *std.Build) void {
    // 1. Common Build Options
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const assets = b.addOptions();
    assets.addOption([]const u8, "app_icon", @embedFile("assets/icon.jpeg"));
    const assets_module = assets.createModule();

    // 2. Define Steps (Modularly)
    // We pass the builder, target, and optimize options to each helper.
    setupOpencut(b, target, optimize, assets_module);
    setupGrayscale(b, target, optimize);
    setupSdlPlayer(b, target, optimize);
    setup3Panels(b, target, optimize, assets_module);
    setupFileTree(b, target, optimize, assets_module);
    setupPlayAudio(b, target, optimize);
    setupMultiWindow(b, target, optimize);
}

// ============================================================
// Helper: FFmpeg Linking (Don't repeat yourself)
// ============================================================
fn linkFFmpeg(exe: *std.Build.Step.Compile) void {
    exe.linkSystemLibrary("avformat");
    exe.linkSystemLibrary("avcodec");
    exe.linkSystemLibrary("avutil");
    exe.linkSystemLibrary("swscale");
}

// ============================================================
// 1. Opencut (Main App)
// ============================================================
fn setupOpencut(b: *std.Build, target: std.Build.ResolvedTarget, optimize: std.builtin.OptimizeMode, assets: *std.Build.Module) void {
    // Lazy load dependency: If this block isn't run, dvui isn't fetched.
    const dvui_dep = b.lazyDependency("dvui", .{
        .target = target,
        .optimize = optimize,
        .backend = .sdl3,
    }) orelse return;

    const exe = b.addExecutable(.{
        .name = "opencut",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/opencut.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });

    // Imports & Linking
    exe.root_module.addImport("dvui", dvui_dep.module("dvui_sdl3"));
    exe.root_module.addImport("SDLBackend", dvui_dep.module("sdl3"));
    exe.root_module.addImport("assets", assets);
    exe.linkLibC();
    exe.linkSystemLibrary("SDL3");
    linkFFmpeg(exe);

    // Build Step: zig build opencut
    const build_step = b.step("opencut", "Build Open Cut");
    // NOTE: using addInstallArtifact here (instead of b.installArtifact) prevents
    // it from being built by default when running "zig build"
    build_step.dependOn(&b.addInstallArtifact(exe, .{}).step);

    // Run Step: zig build run-opencut
    var run_step = b.step("run-opencut", "Run Opencut");
    var run_cmd = b.addRunArtifact(exe);
    if (b.args) |args| run_cmd.addArgs(args);
    run_step.dependOn(&run_cmd.step);
}

// ============================================================
// 2. Grayscale Converter
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
// 3. SDL Player (No DVUI)
// ============================================================
fn setupSdlPlayer(b: *std.Build, target: std.Build.ResolvedTarget, optimize: std.builtin.OptimizeMode) void {
    const exe = b.addExecutable(.{
        .name = "sdl-player",
        .root_module = b.createModule(.{
            .root_source_file = b.path("attempts/sdl-player.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });

    exe.linkLibC();
    exe.linkSystemLibrary("SDL3");
    linkFFmpeg(exe);
    exe.linkSystemLibrary("swresample");

    const build_step = b.step("sdl-player", "Build video player");
    build_step.dependOn(&b.addInstallArtifact(exe, .{}).step);

    const run_step = b.step("run-sdl-player", "Run video player");
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
fn setupMultiWindow(b: *std.Build, target: std.Build.ResolvedTarget, optimize: std.builtin.OptimizeMode) void {
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

    const build_step = b.step("multi-window", "Build multi-window example");
    build_step.dependOn(&b.addInstallArtifact(exe, .{}).step);

    const run_step = b.step("run-multi-window", "Run multi-window example");
    const run_cmd = b.addRunArtifact(exe);
    if (b.args) |args| run_cmd.addArgs(args);
    run_step.dependOn(&run_cmd.step);
}
