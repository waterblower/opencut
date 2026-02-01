const std = @import("std");
const dvui = @import("dvui");
const SDLBackend = @import("SDLBackend");

pub fn main() !void {
    var gpa_instance = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa_instance.deinit();
    const gpa = gpa_instance.allocator();

    var backend = try SDLBackend.initWindow(.{
        .allocator = gpa,
        .size = .{ .w = 600.0, .h = 800.0 },
        .min_size = .{ .w = 400.0, .h = 500.0 },
        .vsync = true,
        .title = "File Tree Explorer - Mock Data",
    });
    defer backend.deinit();

    var win = try dvui.Window.init(@src(), gpa, backend.backend(), .{});
    defer win.deinit();

    const interrupted = false;

    while (true) {
        const nstime = win.beginWait(interrupted);

        try win.begin(nstime);

        try backend.addAllEvents(&win);

        try fileTree(
            @src(),
            ".",
            .{},
            .{},
            .{},
            .{},
        );

        const end_micros = try win.end(.{});

        try backend.setCursor(win.cursorRequested());
        try backend.textInputRect(win.textInputRequested());

        try backend.renderPresent();

        const wait_event_micros = win.waitTime(end_micros);
        _ = try backend.waitEventTimeout(wait_event_micros);
    }
}

const tree_palette = &[_]dvui.Color{
    .{ .r = 0x5e, .g = 0x31, .b = 0x5b, .a = 0xff },
    .{ .r = 0x8c, .g = 0x3f, .b = 0x5d, .a = 0xff },
    .{ .r = 0xba, .g = 0x61, .b = 0x56, .a = 0xff },
    .{ .r = 0xf2, .g = 0xa6, .b = 0x5e, .a = 0xff },
    .{ .r = 0xff, .g = 0xe4, .b = 0x78, .a = 0xff },
    .{ .r = 0xcf, .g = 0xff, .b = 0x70, .a = 0xff },
    .{ .r = 0x8f, .g = 0xde, .b = 0x5d, .a = 0xff },
    .{ .r = 0x3c, .g = 0xa3, .b = 0x70, .a = 0xff },
    .{ .r = 0x3d, .g = 0x6e, .b = 0x70, .a = 0xff },
    .{ .r = 0x32, .g = 0x3e, .b = 0x4f, .a = 0xff },
    .{ .r = 0x32, .g = 0x29, .b = 0x47, .a = 0xff },
    .{ .r = 0x47, .g = 0x3b, .b = 0x78, .a = 0xff },
    .{ .r = 0x4b, .g = 0x5b, .b = 0xab, .a = 0xff },
};

pub fn fileTree(
    src: std.builtin.SourceLocation,
    root_directory: []const u8,
    tree_init_options: dvui.TreeWidget.InitOptions,
    tree_options: dvui.Options,
    branch_options: dvui.Options,
    expander_options: dvui.Options,
) !void {
    var tree = dvui.TreeWidget.tree(src, tree_init_options, tree_options);
    defer tree.deinit();

    const uniqueId = dvui.parentGet().extendId(@src(), 0);
    recurseFiles(root_directory, tree, uniqueId, branch_options, expander_options) catch std.debug.panic("Failed to recurse files", .{});
}

fn recurseFiles(root_directory: []const u8, outer_tree: *dvui.TreeWidget, uniqueId: dvui.Id, branch_options: dvui.Options, expander_options: dvui.Options) !void {
    const recursor = struct {
        fn search(directory: []const u8, tree: *dvui.TreeWidget, uid: dvui.Id, color_id: *usize, branch_opts: dvui.Options, expander_opts: dvui.Options) !void {
            var dir = std.fs.cwd().openDir(directory, .{ .access_sub_paths = true, .iterate = true }) catch return;
            defer dir.close();

            const padding = dvui.Rect.all(2);

            var iter = dir.iterate();

            var id_extra: usize = 0;
            while (try iter.next()) |entry| {
                id_extra += 1;

                var branch_opts_override = dvui.Options{
                    .id_extra = id_extra,
                    .expand = .horizontal,
                };

                const color = tree_palette[color_id.* % tree_palette.len];

                const branch = tree.branch(@src(), .{
                    .expanded = false,
                }, branch_opts_override.override(branch_opts));
                defer branch.deinit();

                const abs_path = try std.fs.path.join(
                    dvui.currentWindow().arena(),
                    &.{ directory, entry.name },
                );

                if (branch.insertBefore()) {
                    if (dvui.dataGetSlice(null, uid, "removed_path", []u8)) |removed_path| {
                        const old_sub_path = std.fs.path.basename(removed_path);

                        const new_path = try std.fs.path.join(dvui.currentWindow().arena(), &.{ if (entry.kind == .directory) abs_path else directory, old_sub_path });

                        if (!std.mem.eql(u8, removed_path, new_path)) {
                            std.log.debug("DVUI/TreeWidget: Moved {s} to {s}", .{ removed_path, new_path });

                            try std.fs.renameAbsolute(removed_path, new_path);
                        }

                        dvui.dataRemove(null, uid, "removed_path");
                    }
                }

                if (branch.floating()) {
                    if (dvui.dataGetSlice(null, uid, "removed_path", []u8) == null)
                        dvui.dataSetSlice(null, uid, "removed_path", abs_path);
                }

                switch (entry.kind) {
                    .file => {
                        const icon = dvui.entypo.text_document;
                        const icon_color = color;
                        const text_color = dvui.themeGet().color(.control, .text);

                        _ = dvui.icon(
                            @src(),
                            "FileIcon",
                            icon,
                            .{ .fill_color = icon_color },
                            .{
                                .gravity_y = 0.5,
                                .padding = padding,
                            },
                        );
                        dvui.label(
                            @src(),
                            "{s}",
                            .{entry.name},
                            .{
                                .color_text = text_color,
                                .padding = padding,
                            },
                        );

                        if (branch.button.clicked()) {
                            std.log.debug("Clicked: {s}", .{abs_path});
                        }
                    },
                    .directory => {
                        const folder_name = std.fs.path.basename(abs_path);
                        const icon_color = color;

                        _ = dvui.icon(
                            @src(),
                            "FolderIcon",
                            dvui.entypo.folder,
                            .{
                                .fill_color = icon_color,
                            },
                            .{
                                .gravity_y = 0.5,
                                .padding = padding,
                            },
                        );
                        dvui.label(@src(), "{s}", .{folder_name}, .{
                            .color_text = dvui.themeGet().color(.control, .text),
                            .padding = padding,
                        });
                        _ = dvui.icon(
                            @src(),
                            "DropIcon",
                            if (branch.expanded) dvui.entypo.triangle_down else dvui.entypo.triangle_right,
                            .{ .fill_color = icon_color },
                            .{
                                .gravity_y = 0.5,
                                .gravity_x = 1.0,
                                .padding = padding,
                            },
                        );

                        var expander_opts_override = dvui.Options{
                            .margin = .{ .x = 14 },
                            .color_border = color,
                            .expand = .horizontal,
                        };

                        if (branch.expander(@src(), .{ .indent = 14 }, expander_opts_override.override(expander_opts))) {
                            try search(
                                abs_path,
                                tree,
                                uid,
                                color_id,
                                branch_opts,
                                expander_opts,
                            );
                        }
                        color_id.* = color_id.* + 1;
                    },
                    else => {},
                }
            }
        }
    }.search;

    var color_index: usize = 0;

    const root_branch = outer_tree.branch(@src(), .{
        .expanded = true,
    }, .{
        .id_extra = 0,
        .expand = .horizontal,
        //.color_fill_hover = .fill,
    });
    defer root_branch.deinit();

    dvui.icon(
        @src(),
        "FolderIcon",
        dvui.entypo.folder,
        .{
            .fill_color = tree_palette[0],
        },
        .{
            .gravity_y = 0.5,
            .padding = dvui.Rect.all(10),
        },
    );

    const folder_name = std.fs.path.basename(root_directory);
    dvui.label(@src(), "{s}", .{folder_name}, .{
        .color_text = dvui.themeGet().color(.control, .text),
        .padding = dvui.Rect.all(10),
    });
    dvui.icon(
        @src(),
        "DropIcon",
        if (root_branch.expanded) dvui.entypo.triangle_down else dvui.entypo.triangle_right,
        .{ .fill_color = tree_palette[0] },
        .{
            .gravity_y = 0.5,
            .gravity_x = 1.0,
            .padding = dvui.Rect.all(10),
        },
    );

    if (root_branch.expander(@src(), .{ .indent = 14.0 }, .{
        .color_fill = dvui.themeGet().color(.window, .fill),
        .color_border = tree_palette[0],
        .expand = .horizontal,
        .corner_radius = root_branch.button.wd.options.corner_radius,
        .background = true,
        .border = .{ .x = 1 },
        .box_shadow = .{
            .color = .black,
            .offset = .{ .x = -5, .y = 5 },
            .shrink = 5,
            .fade = 10,
            .alpha = 0.15,
        },
    })) {
        try recursor(root_directory, outer_tree, uniqueId, &color_index, branch_options, expander_options);
    }

    return;
}
