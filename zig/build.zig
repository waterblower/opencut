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

    {
        const dvui_dep = b.dependency("dvui", .{
            .target = target,
            .optimize = optimize,
            .backend = .sdl3,
        });

        // Create opencut executable
        const opencut_exe = b.addExecutable(.{
            .name = "opencut",
            .root_module = b.createModule(.{
                .root_source_file = b.path("src/opencut.zig"),
                .target = target,
                .optimize = optimize,
            }),
        });

        opencut_exe.root_module.addImport("dvui", dvui_dep.module("dvui_sdl3"));
        opencut_exe.root_module.addImport("SDLBackend", dvui_dep.module("sdl3"));

        opencut_exe.linkLibC();
        opencut_exe.linkSystemLibrary("SDL3");
        opencut_exe.linkSystemLibrary("avformat");
        opencut_exe.linkSystemLibrary("avcodec");
        opencut_exe.linkSystemLibrary("avutil");
        opencut_exe.linkSystemLibrary("swscale");

        b.installArtifact(opencut_exe);

        const build_opencut_step = b.step("opencut", "Build Open Cut");
        build_opencut_step.dependOn(&b.addInstallArtifact(opencut_exe, .{}).step);

        const run_opencut = b.step("run-opencut", "Run the DVUI video player");
        const opencut_cmd = b.addRunArtifact(opencut_exe);
        run_opencut.dependOn(&opencut_cmd.step);

        opencut_cmd.step.dependOn(b.getInstallStep());

        if (b.args) |args| {
            opencut_cmd.addArgs(args);
        }
    }

    // Create grayscale converter executable
    {
        const grayscale_exe = b.addExecutable(.{
            .name = "grayscale",
            .root_module = b.createModule(.{
                .root_source_file = b.path("attempts/grayscale.zig"),
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

        const build_grayscale_step = b.step("grayscale", "Build only the grayscale converter");
        build_grayscale_step.dependOn(&b.addInstallArtifact(grayscale_exe, .{}).step);

        // Create run step for grayscale converter
        const run_grayscale_step = b.step("run-grayscale", "Run the grayscale converter");
        const grayscale_run_cmd = b.addRunArtifact(grayscale_exe);
        run_grayscale_step.dependOn(&grayscale_run_cmd.step);
        grayscale_run_cmd.step.dependOn(b.getInstallStep());

        if (b.args) |args| {
            grayscale_run_cmd.addArgs(args);
        }
    }

    // Create player executable
    {
        const player_exe = b.addExecutable(.{
            .name = "sdl-player",
            .root_module = b.createModule(.{
                .root_source_file = b.path("attempts/sdl-player.zig"),
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

        const build_player_step = b.step("sdl-player", "Build only the video player");
        build_player_step.dependOn(&b.addInstallArtifact(player_exe, .{}).step);

        // Create run step for player
        const run_player_step = b.step("run-sdl-player", "Run the video player");
        const player_run_cmd = b.addRunArtifact(player_exe);
        run_player_step.dependOn(&player_run_cmd.step);
        player_run_cmd.step.dependOn(b.getInstallStep());

        if (b.args) |args| {
            player_run_cmd.addArgs(args);
        }
    }

    // Create 3-panels example executable
    {
        const dvui_dep = b.dependency("dvui", .{
            .target = target,
            .optimize = optimize,
            .backend = .sdl3,
        });

        const panels_exe = b.addExecutable(.{
            .name = "3-panels",
            .root_module = b.createModule(.{
                .root_source_file = b.path("attempts/3-panels.zig"),
                .target = target,
                .optimize = optimize,
            }),
        });

        panels_exe.root_module.addImport("dvui", dvui_dep.module("dvui_sdl3"));
        panels_exe.root_module.addImport("SDLBackend", dvui_dep.module("sdl3"));

        panels_exe.linkLibC();
        panels_exe.linkSystemLibrary("SDL3");

        b.installArtifact(panels_exe);

        const build_panels_step = b.step("3-panels", "Build 3-panel IDE layout example");
        build_panels_step.dependOn(&b.addInstallArtifact(panels_exe, .{}).step);

        const run_panels_step = b.step("run-3-panels", "Run the 3-panel IDE layout example");
        const panels_run_cmd = b.addRunArtifact(panels_exe);
        run_panels_step.dependOn(&panels_run_cmd.step);
        panels_run_cmd.step.dependOn(b.getInstallStep());

        if (b.args) |args| {
            panels_run_cmd.addArgs(args);
        }
    }

    {
        const dvui_dep = b.dependency("dvui", .{
            .target = target,
            .optimize = optimize,
            .backend = .sdl3,
        });

        const panels_exe = b.addExecutable(.{
            .name = "file-tree",
            .root_module = b.createModule(.{
                .root_source_file = b.path("attempts/file-tree.zig"),
                .target = target,
                .optimize = optimize,
            }),
        });

        panels_exe.root_module.addImport("dvui", dvui_dep.module("dvui_sdl3"));
        panels_exe.root_module.addImport("SDLBackend", dvui_dep.module("sdl3"));

        panels_exe.linkLibC();
        panels_exe.linkSystemLibrary("SDL3");

        b.installArtifact(panels_exe);

        const build_panels_step = b.step("file-tree", "Build 3-panel IDE layout example");
        build_panels_step.dependOn(&b.addInstallArtifact(panels_exe, .{}).step);

        const run_panels_step = b.step("run-file-tree", "Run the 3-panel IDE layout example");
        const panels_run_cmd = b.addRunArtifact(panels_exe);
        run_panels_step.dependOn(&panels_run_cmd.step);
        panels_run_cmd.step.dependOn(b.getInstallStep());

        if (b.args) |args| {
            panels_run_cmd.addArgs(args);
        }
    }

    // Create play-audio executable
    {
        const zaudio_dep = b.dependency("zaudio", .{
            .target = target,
            .optimize = optimize,
        });

        const audio_exe = b.addExecutable(.{
            .name = "play-audio",
            .root_module = b.createModule(.{
                .root_source_file = b.path("attempts/play-audio.zig"),
                .target = target,
                .optimize = optimize,
            }),
        });

        audio_exe.root_module.addImport("zaudio", zaudio_dep.module("root"));
        audio_exe.linkLibrary(zaudio_dep.artifact("miniaudio"));

        b.installArtifact(audio_exe);

        const build_audio_step = b.step("play-audio", "Build the audio player");
        build_audio_step.dependOn(&b.addInstallArtifact(audio_exe, .{}).step);

        const run_audio_step = b.step("run-play-audio", "Run the audio player");
        const audio_run_cmd = b.addRunArtifact(audio_exe);
        run_audio_step.dependOn(&audio_run_cmd.step);
        audio_run_cmd.step.dependOn(b.getInstallStep());

        if (b.args) |args| {
            audio_run_cmd.addArgs(args);
        }
    }
}
