const std = @import("std");
const builtin = @import("builtin");
const dvui = @import("dvui");
const enums = dvui.enums;

var progress_mutex = std.Thread.Mutex{};
var progress_val: f32 = 0.0;

const window_icon_png = @embedFile("zig-favicon.png");

// To be a dvui App:
// * declare "dvui_app"
// * expose the backend's main function
// * use the backend's log function
pub const dvui_app: dvui.App = .{
    .config = .{
        .options = .{
            .size = .{ .w = 800.0, .h = 600.0 },
            .min_size = .{ .w = 250.0, .h = 350.0 },
            .title = "DVUI App Example",
            .icon = window_icon_png,
            .window_init_options = .{
                // Could set a default theme here
                // .theme = dvui.Theme.builtin.dracula,
            },
        },
    },
    .frameFn = AppFrame,
    .initFn = AppInit,
    .deinitFn = AppDeinit,
};
pub const main = dvui.App.main;
pub const panic = dvui.App.panic;
pub const std_options: std.Options = .{
    .logFn = dvui.App.logFn,
};

var gpa_instance = std.heap.GeneralPurposeAllocator(.{}){};
const gpa = gpa_instance.allocator();

var orig_content_scale: f32 = 1.0;
var warn_on_quit: bool = false;
var warn_on_quit_closing: bool = false;

// Runs before the first frame, after backend and dvui.Window.init()
// - runs between win.begin()/win.end()
pub fn AppInit(win: *dvui.Window) !void {
    orig_content_scale = win.content_scale;

    // Add your own bundled font files...:
    // try dvui.addFont("NOTO", @embedFile("../src/fonts/NotoSansKR-Regular.ttf"), null);

    if (false) {
        // If you need to set a theme based on the users preferred color scheme, do it here
        const theme = switch (win.backend.preferredColorScheme() orelse .light) {
            .light => dvui.Theme.builtin.adwaita_light,
            .dark => dvui.Theme.builtin.adwaita_dark,
        };

        win.themeSet(theme);
    }
}

// Run as app is shutting down before dvui.Window.deinit()
pub fn AppDeinit() void {}

// Run each frame to do normal UI
pub fn AppFrame() !dvui.App.Result {
    var scaler = dvui.scale(@src(), .{ .scale = &dvui.currentWindow().content_scale, .pinch_zoom = .global }, .{ .rect = .cast(dvui.windowRect()) });
    scaler.deinit();

    {
        var hbox = dvui.box(@src(), .{ .dir = .horizontal }, .{ .style = .window, .background = true, .expand = .horizontal });
        defer hbox.deinit();

        var m = dvui.menu(@src(), .horizontal, .{});
        defer m.deinit();

        if (dvui.menuItemLabel(@src(), "File", .{ .submenu = true }, .{ .tag = "first-focusable" })) |r| {
            var fw = dvui.floatingMenu(@src(), .{ .from = r }, .{});
            defer fw.deinit();

            if (dvui.menuItemLabel(@src(), "Close Menu", .{}, .{ .expand = .horizontal }) != null) {
                m.close();
            }

            if (dvui.backend.kind != .web) {
                if (dvui.menuItemLabel(@src(), "Exit", .{}, .{ .expand = .horizontal }) != null) {
                    return .close;
                }
            }
        }
    }

    var scroll = dvui.scrollArea(@src(), .{}, .{ .expand = .both, .style = .window });
    defer scroll.deinit();

    var tl = dvui.textLayout(@src(), .{}, .{ .expand = .horizontal, .font = .theme(.title) });
    const lorem = "This is a dvui.App example that can compile on multiple backends.\n";
    tl.addText(lorem, .{});
    tl.format("Current backend: {s}", .{@tagName(dvui.backend.kind)}, .{});
    if (dvui.backend.kind == .web) {
        tl.format(" : {s}", .{if (dvui.backend.wasm.wasm_about_webgl2() == 1) "webgl2" else "webgl (no mipmaps)"}, .{});
    }
    tl.deinit();

    var tl2 = dvui.textLayout(@src(), .{}, .{ .expand = .horizontal });
    tl2.addText(
        \\DVUI
        \\- paints the entire window
        \\- can show floating windows and dialogs
        \\- rest of the window is a scroll area
        \\
        \\
    , .{});
    tl2.addText("Framerate is variable and adjusts as needed for input events and animations.\n\n", .{});
    tl2.addText("Framerate is capped by vsync.\n\n", .{});
    tl2.addText("Cursor is always being set by dvui.\n\n", .{});
    if (dvui.useFreeType) {
        tl2.addText("Fonts are being rendered by FreeType 2.", .{});
    } else {
        tl2.addText("Fonts are being rendered by stb_truetype.", .{});
    }
    tl2.deinit();

    const label = if (dvui.Examples.show_demo_window) "Hide Demo Window" else "Show Demo Window";
    if (dvui.button(@src(), label, .{}, .{ .tag = "show-demo-btn" })) {
        dvui.Examples.show_demo_window = !dvui.Examples.show_demo_window;
    }

    if (dvui.button(@src(), "Debug Window", .{}, .{})) {
        dvui.toggleDebugWindow();
    }

    {
        var hbox = dvui.box(@src(), .{ .dir = .horizontal }, .{});
        defer hbox.deinit();
        dvui.label(@src(), "Pinch Zoom or Scale", .{}, .{});
        if (dvui.buttonIcon(@src(), "plus", dvui.entypo.plus, .{}, .{}, .{})) {
            dvui.currentWindow().content_scale *= 1.1;
        }

        if (dvui.buttonIcon(@src(), "minus", dvui.entypo.minus, .{}, .{}, .{})) {
            dvui.currentWindow().content_scale /= 1.1;
        }

        if (dvui.currentWindow().content_scale != orig_content_scale) {
            if (dvui.button(@src(), "Reset Scale", .{}, .{})) {
                dvui.currentWindow().content_scale = orig_content_scale;
            }
        }
    }

    if (dvui.backend.kind != .web) {
        _ = dvui.checkbox(@src(), &warn_on_quit, "Warn on Quit", .{});

        if (warn_on_quit) {
            if (warn_on_quit_closing) return .close;

            const wd = dvui.currentWindow().data();
            for (dvui.events()) |*e| {
                if (!dvui.eventMatchSimple(e, wd)) continue;

                if ((e.evt == .window and e.evt.window.action == .close) or (e.evt == .app and e.evt.app.action == .quit)) {
                    e.handle(@src(), wd);

                    const warnAfter: dvui.DialogCallAfterFn = struct {
                        fn warnAfter(_: dvui.Id, response: dvui.enums.DialogResponse) !void {
                            if (response == .ok) warn_on_quit_closing = true;
                        }
                    }.warnAfter;

                    dvui.dialog(@src(), .{}, .{ .message = "Really Quit?", .cancel_label = "Cancel", .callafterFn = warnAfter });
                }
            }
        }
    }

    // look at demo() for examples of dvui widgets, shows in a floating window
    dvui.Examples.demo();

    return .ok;
}

pub fn dialogs(demo_win_id: dvui.Id) void {
    {
        var hbox = dvui.box(@src(), .{ .dir = .horizontal }, .{});
        defer hbox.deinit();

        if (dvui.button(@src(), "Non modal", .{}, .{})) {
            dvui.dialog(@src(), .{}, .{ .modal = false, .title = "Ok Dialog", .ok_label = "Ok", .message = "This is a non modal dialog with no callafter\n\nThe ok button is focused by default" });
        }

        const dialogsFollowup = struct {
            fn callafter(id: dvui.Id, response: enums.DialogResponse) !void {
                _ = id;
                var buf: [100]u8 = undefined;
                const text = std.fmt.bufPrint(&buf, "You clicked \"{s}\" in the previous dialog", .{@tagName(response)}) catch unreachable;
                dvui.dialog(@src(), .{}, .{ .title = "Ok Followup Response", .message = text });
            }
        };

        if (dvui.button(@src(), "Modal with followup", .{}, .{})) {
            dvui.dialog(@src(), .{}, .{ .title = "Followup", .message = "This is a modal dialog with modal followup\n\nHere the cancel button is focused", .callafterFn = dialogsFollowup.callafter, .cancel_label = "Cancel", .default = .cancel });
        }
    }

    {
        var hbox = dvui.box(@src(), .{ .dir = .horizontal }, .{});
        defer hbox.deinit();

        if (dvui.button(@src(), "Toast 1", .{}, .{})) {
            dvui.toast(@src(), .{ .subwindow_id = demo_win_id, .message = "Toast 1 to demo window" });
        }

        if (dvui.button(@src(), "Toast 2", .{}, .{})) {
            dvui.toast(@src(), .{ .subwindow_id = demo_win_id, .message = "Toast 2" });
        }

        if (dvui.button(@src(), "Toast 3", .{}, .{})) {
            dvui.toast(@src(), .{ .subwindow_id = demo_win_id, .message = "Toast 3 is really really long to demo window" });
        }

        if (dvui.button(@src(), "Toast Main Window", .{}, .{})) {
            dvui.toast(@src(), .{ .message = "Toast to main window" });
        }
    }

    {
        var vbox = dvui.box(@src(), .{}, .{ .min_size_content = .{ .w = 250, .h = 80 }, .border = .all(1) });
        defer vbox.deinit();

        if (dvui.button(@src(), "Toast In Box", .{}, .{})) {
            dvui.toast(@src(), .{ .subwindow_id = vbox.data().id, .message = "Toast to this box" });
        }

        dvui.toastsShow(vbox.data().id, vbox.data().contentRectScale().r.toNatural());
    }

    dvui.label(@src(), "\nDialogs and toasts from other threads", .{}, .{});
    {
        var hbox = dvui.box(@src(), .{ .dir = .horizontal }, .{});
        defer hbox.deinit();

        if (dvui.button(@src(), "Dialog after 1 second", .{}, .{})) {
            if (!builtin.single_threaded) blk: {
                const bg_thread = std.Thread.spawn(.{}, background_dialog, .{ dvui.currentWindow(), 1_000_000_000 }) catch |err| {
                    dvui.log.debug("Failed to spawn background thread for delayed action, got {any}", .{err});
                    break :blk;
                };
                bg_thread.detach();
            } else {
                dvui.toast(@src(), .{ .subwindow_id = demo_win_id, .message = "Not available in single-threaded" });
            }
        }

        if (dvui.button(@src(), "Toast after 1 second", .{}, .{})) {
            if (!builtin.single_threaded) blk: {
                const bg_thread = std.Thread.spawn(.{}, background_toast, .{ dvui.currentWindow(), 1_000_000_000, demo_win_id }) catch |err| {
                    dvui.log.debug("Failed to spawn background thread for delayed action, got {any}", .{err});
                    break :blk;
                };
                bg_thread.detach();
            } else {
                dvui.toast(@src(), .{ .subwindow_id = demo_win_id, .message = "Not available in single-threaded" });
            }
        }
    }

    {
        var hbox = dvui.box(@src(), .{ .dir = .horizontal }, .{ .expand = .horizontal });
        defer hbox.deinit();

        if (dvui.button(@src(), "Show Progress from another Thread", .{}, .{})) {
            progress_mutex.lock();
            progress_val = 0;
            progress_mutex.unlock();
            if (!builtin.single_threaded) blk: {
                const bg_thread = std.Thread.spawn(.{}, background_progress, .{ dvui.currentWindow(), 2_000_000_000 }) catch |err| {
                    dvui.log.debug("Failed to spawn background thread for delayed action, got {any}", .{err});
                    break :blk;
                };
                bg_thread.detach();
            } else {
                dvui.toast(@src(), .{ .subwindow_id = demo_win_id, .message = "Not available in single-threaded" });
            }
        }

        dvui.progress(@src(), .{ .percent = progress_val }, .{ .expand = .horizontal, .gravity_y = 0.5, .corner_radius = dvui.Rect.all(100) });
    }

    dvui.label(@src(), "\nNative Dialogs", .{}, .{});
    {
        var hbox = dvui.box(@src(), .{ .dir = .horizontal }, .{});
        defer hbox.deinit();

        const single_file_id = hbox.widget().extendId(@src(), 0);

        if (dvui.button(@src(), "Open File", .{}, .{})) {
            if (dvui.backend.kind == .web) {
                dvui.dialogWasmFileOpen(single_file_id, .{ .accept = ".png, .jpg" });
            } else if (!dvui.useTinyFileDialogs) {
                dvui.toast(@src(), .{ .subwindow_id = demo_win_id, .message = "Tiny File Dilaogs disabled" });
            } else {
                const filename = dvui.dialogNativeFileOpen(dvui.currentWindow().arena(), .{
                    .title = "dvui native file open",
                    .filters = &.{ "*.png", "*.jpg" },
                    .filter_description = "images",
                }) catch |err| blk: {
                    dvui.log.debug("Could not open file dialog, got {any}", .{err});
                    break :blk null;
                };
                if (filename) |f| {
                    dvui.dialog(@src(), .{}, .{ .modal = false, .title = "File Open Result", .ok_label = "Done", .message = f });
                }
            }
        }

        if (dvui.wasmFileUploaded(single_file_id)) |file| {
            dvui.dialog(@src(), .{}, .{ .modal = false, .title = "File Open Result", .ok_label = "Done", .message = file.name });
        }

        const multi_file_id = hbox.widget().extendId(@src(), 0);

        if (dvui.button(@src(), "Open Multiple Files", .{}, .{})) {
            if (dvui.backend.kind == .web) {
                dvui.dialogWasmFileOpenMultiple(multi_file_id, .{ .accept = ".png, .jpg" });
            } else if (!dvui.useTinyFileDialogs) {
                dvui.toast(@src(), .{ .subwindow_id = demo_win_id, .message = "Tiny File Dilaogs disabled" });
            } else {
                const filenames = dvui.dialogNativeFileOpenMultiple(dvui.currentWindow().arena(), .{
                    .title = "dvui native file open multiple",
                    .filter_description = "images",
                }) catch |err| blk: {
                    dvui.log.debug("Could not open multi file dialog, got {any}", .{err});
                    break :blk null;
                };
                if (filenames) |files| {
                    const msg = std.mem.join(dvui.currentWindow().lifo(), "\n", files) catch "";
                    defer dvui.currentWindow().lifo().free(msg);
                    dvui.dialog(@src(), .{}, .{ .modal = false, .title = "File Open Multiple Result", .ok_label = "Done", .message = msg });
                }
            }
        }

        if (dvui.wasmFileUploadedMultiple(multi_file_id)) |files| blk: {
            const lifo = dvui.currentWindow().lifo();
            const names = lifo.alloc([:0]const u8, files.len) catch break :blk;
            defer dvui.currentWindow().lifo().free(names);
            for (files, names) |f, *name| name.* = f.name;

            const msg = std.mem.join(dvui.currentWindow().lifo(), "\n", names) catch "";
            defer dvui.currentWindow().lifo().free(msg);
            dvui.dialog(@src(), .{}, .{ .modal = false, .title = "File Open Multiple Result", .ok_label = "Done", .message = msg });
        }
    }
    {
        var hbox = dvui.box(@src(), .{ .dir = .horizontal }, .{});
        defer hbox.deinit();

        if (dvui.button(@src(), "Open Folder", .{}, .{})) {
            if (dvui.backend.kind == .web) {
                dvui.toast(@src(), .{ .subwindow_id = demo_win_id, .message = "Not implemented for web" });
            } else if (!dvui.useTinyFileDialogs) {
                dvui.toast(@src(), .{ .subwindow_id = demo_win_id, .message = "Tiny File Dilaogs disabled" });
            } else {
                const filename = dvui.dialogNativeFolderSelect(dvui.currentWindow().arena(), .{ .title = "dvui native folder select" }) catch |err| blk: {
                    dvui.log.debug("Could not open folder select dialog, got {any}", .{err});
                    break :blk null;
                };
                if (filename) |f| {
                    dvui.dialog(@src(), .{}, .{ .modal = false, .title = "Folder Select Result", .ok_label = "Done", .message = f });
                }
            }
        }

        if (dvui.button(@src(), "Save File", .{}, .{})) {
            if (dvui.backend.kind == .web) {
                dvui.dialog(@src(), .{}, .{ .modal = false, .title = "Save File", .ok_label = "Ok", .message = "Not available on the web.  For file download, see \"Save Plot\" in the plots example." });
            } else if (!dvui.useTinyFileDialogs) {
                dvui.toast(@src(), .{ .subwindow_id = demo_win_id, .message = "Tiny File Dilaogs disabled" });
            } else {
                const filename = dvui.dialogNativeFileSave(dvui.currentWindow().arena(), .{ .title = "dvui native file save" }) catch |err| blk: {
                    dvui.log.debug("Could not open file save dialog, got {any}", .{err});
                    break :blk null;
                };
                if (filename) |f| {
                    dvui.dialog(@src(), .{}, .{ .modal = false, .title = "File Save Result", .ok_label = "Done", .message = f });
                }
            }
        }
    }
}

fn background_dialog(win: *dvui.Window, delay_ns: u64) void {
    std.Thread.sleep(delay_ns);
    dvui.dialog(@src(), .{}, .{ .window = win, .modal = false, .title = "Background Dialog", .message = "This non modal dialog was added from a non-GUI thread." });
}

fn background_toast(win: *dvui.Window, delay_ns: u64, subwindow_id: ?dvui.Id) void {
    std.Thread.sleep(delay_ns);
    dvui.refresh(win, @src(), null);
    dvui.toast(@src(), .{ .window = win, .subwindow_id = subwindow_id, .message = "Toast came from a non-GUI thread" });
}

fn background_progress(win: *dvui.Window, delay_ns: u64) void {
    const interval: u64 = 10_000_000;
    var total_sleep: u64 = 0;
    while (total_sleep < delay_ns) : (total_sleep += interval) {
        std.Thread.sleep(interval);
        progress_mutex.lock();
        progress_val = @as(f32, @floatFromInt(total_sleep)) / @as(f32, @floatFromInt(delay_ns));
        progress_mutex.unlock();
        dvui.refresh(win, @src(), null);
    }
}

test {
    @import("std").testing.refAllDecls(@This());
}

test "DOCIMG dialogs" {
    var t = try dvui.testing.init(.{ .window_size = .{ .w = 400, .h = 300 } });
    defer t.deinit();

    const frame = struct {
        fn frame() !dvui.App.Result {
            var box = dvui.box(@src(), .{}, .{ .expand = .both, .background = true, .style = .window });
            defer box.deinit();
            dialogs(box.data().id);
            return .ok;
        }
    }.frame;

    try dvui.testing.settle(frame);

    // Tab to the main window toast button
    for (0..8) |_| {
        try dvui.testing.pressKey(.tab, .none);
        _ = try dvui.testing.step(frame);
    }
    try dvui.testing.pressKey(.enter, .none);

    try dvui.testing.settle(frame);
    try t.saveImage(frame, null, "Examples-dialogs.png");
}
