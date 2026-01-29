const std = @import("std");

// Although this function looks imperative, it does not perform the build
// directly and instead it mutates the build graph (`b`) that will be then
// executed by an external runner. The functions in `std.Build` implement a DSL
// for defining build steps and express dependencies between them, allowing the
// build runner to parallelize the build automatically (and the cache system to
// know when a step doesn't need to be re-run).
pub fn build(b: *std.Build) void {
    // Standard target options allow the person running `zig build` to choose
    // what target to build for. Here we do not override the defaults, which
    // means any target is allowed, and the default is native. Other options
    // for restricting supported target set are available.
    const target = b.standardTargetOptions(.{});
    // Standard optimization options allow the person running `zig build` to select
    // between Debug, ReleaseSafe, ReleaseFast, and ReleaseSmall. Here we do not
    // set a preferred release mode, allowing the user to decide how to optimize.
    const optimize = b.standardOptimizeOption(.{});

    // Import dvui dependency with SDL3 backend
    const dvui_dep = b.dependency("dvui", .{
        .target = target,
        .optimize = optimize,
        .backend = .sdl3,
    });
    // It's also possible to define more custom flags to toggle optional features
    // of this build script using `b.option()`. All defined flags (including
    // target and optimize options) will be listed when running `zig build --help`
    // in this directory.

    // Create grayscale converter executable
    const grayscale_exe = b.addExecutable(.{
        .name = "grayscale",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/grayscale.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });

    grayscale_exe.linkLibC();
    grayscale_exe.linkSystemLibrary("avformat");
    grayscale_exe.linkSystemLibrary("avcodec");
    grayscale_exe.linkSystemLibrary("avutil");
    grayscale_exe.linkSystemLibrary("swscale");

    b.installArtifact(grayscale_exe);

    // Create SDL window executable
    const sdl_window_exe = b.addExecutable(.{
        .name = "sdl-window",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/sdl-window.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });

    sdl_window_exe.linkLibC();
    sdl_window_exe.linkSystemLibrary("SDL3");

    b.installArtifact(sdl_window_exe);

    // Create player executable
    const player_exe = b.addExecutable(.{
        .name = "player",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/player.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });

    player_exe.linkLibC();
    player_exe.linkSystemLibrary("SDL3");
    player_exe.linkSystemLibrary("avformat");
    player_exe.linkSystemLibrary("avcodec");
    player_exe.linkSystemLibrary("avutil");
    player_exe.linkSystemLibrary("swscale");
    player_exe.linkSystemLibrary("swresample");

    b.installArtifact(player_exe);

    // Create build steps for individual executables
    const build_grayscale_step = b.step("grayscale", "Build only the grayscale converter");
    build_grayscale_step.dependOn(&b.addInstallArtifact(grayscale_exe, .{}).step);

    const build_sdl_window_step = b.step("sdl-window", "Build only the SDL window demo");
    build_sdl_window_step.dependOn(&b.addInstallArtifact(sdl_window_exe, .{}).step);

    const build_player_step = b.step("player", "Build only the video player");
    build_player_step.dependOn(&b.addInstallArtifact(player_exe, .{}).step);

    // Create run step for grayscale converter
    const run_grayscale_step = b.step("run-grayscale", "Run the grayscale converter");
    const grayscale_run_cmd = b.addRunArtifact(grayscale_exe);
    run_grayscale_step.dependOn(&grayscale_run_cmd.step);
    grayscale_run_cmd.step.dependOn(b.getInstallStep());

    // Create run step for SDL window demo
    const run_sdl_window_step = b.step("run-sdl-window", "Run the SDL window demo");
    const sdl_window_run_cmd = b.addRunArtifact(sdl_window_exe);
    run_sdl_window_step.dependOn(&sdl_window_run_cmd.step);
    sdl_window_run_cmd.step.dependOn(b.getInstallStep());

    // Create run step for player
    const run_player_step = b.step("run-player", "Run the video player");
    const player_run_cmd = b.addRunArtifact(player_exe);
    run_player_step.dependOn(&player_run_cmd.step);
    player_run_cmd.step.dependOn(b.getInstallStep());

    {
        // Create DVUI video player executable
        const dvui_exe = b.addExecutable(.{
            .name = "dvui-player",
            .root_module = b.createModule(.{
                .root_source_file = b.path("src/dvui.zig"),
                .target = target,
                .optimize = optimize,
            }),
        });

        dvui_exe.root_module.addImport("dvui", dvui_dep.module("dvui_sdl3"));
        dvui_exe.root_module.addImport("SDLBackend", dvui_dep.module("sdl3"));

        dvui_exe.linkLibC();
        dvui_exe.linkSystemLibrary("SDL3");
        dvui_exe.linkSystemLibrary("avformat");
        dvui_exe.linkSystemLibrary("avcodec");
        dvui_exe.linkSystemLibrary("avutil");
        dvui_exe.linkSystemLibrary("swscale");

        b.installArtifact(dvui_exe);

        const build_dvui_step = b.step("dvui", "Build only the DVUI video player");
        build_dvui_step.dependOn(&b.addInstallArtifact(dvui_exe, .{}).step);

        // Create run step for DVUI player
        const run_dvui_step = b.step("run-dvui", "Run the DVUI video player");
        const dvui_run_cmd = b.addRunArtifact(dvui_exe);
        run_dvui_step.dependOn(&dvui_run_cmd.step);
        dvui_run_cmd.step.dependOn(b.getInstallStep());

        if (b.args) |args| {
            dvui_run_cmd.addArgs(args);
        }
    }

    if (b.args) |args| {
        grayscale_run_cmd.addArgs(args);
        sdl_window_run_cmd.addArgs(args);
        player_run_cmd.addArgs(args);
    }

    // Just like flags, top level steps are also listed in the `--help` menu.
    //
    // The Zig build system is entirely implemented in userland, which means
    // that it cannot hook into private compiler APIs. All compilation work
    // orchestrated by the build system will result in other Zig compiler
    // subcommands being invoked with the right flags defined. You can observe
    // these invocations when one fails (or you pass a flag to increase
    // verbosity) to validate assumptions and diagnose problems.
    //
    // Lastly, the Zig build system is relatively simple and self-contained,
    // and reading its source code will allow you to master it.
}
