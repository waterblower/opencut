const std = @import("std");
const sdl = @import("sdl"); // 假设你在 build.zig 里配置了 sdl 模块
const c = sdl.c; // 使用 sdl 包里的 cImport
const sleep = std.Thread.sleep;

// FFmpeg C Imports (如果 sdl.c 里没有包含这些，需要手动引入)
const ffmpeg = @cImport({
    @cInclude("libavformat/avformat.h");
    @cInclude("libavcodec/avcodec.h");
    @cInclude("libswresample/swresample.h");
    @cInclude("libavutil/opt.h");
    @cInclude("libavutil/channel_layout.h");
    @cInclude("libavutil/samplefmt.h");
});

pub fn play_audio(filename: []const u8) !void {
    ffmpeg.av_log_set_level(ffmpeg.AV_LOG_ERROR);
    // 2. 初始化 SDL3 音频子系统
    if (c.SDL_Init(c.SDL_INIT_AUDIO) == false) {
        std.debug.print("SDL_Init failed: {s}\n", .{c.SDL_GetError()});
        return error.SDLInitFailed;
    }
    defer c.SDL_Quit();

    // 3. 打开 FFmpeg 文件
    var fmt_ctx: ?*ffmpeg.AVFormatContext = null;
    if (ffmpeg.avformat_open_input(&fmt_ctx, filename.ptr, null, null) < 0) {
        return error.CouldNotOpenFile;
    }
    defer ffmpeg.avformat_close_input(&fmt_ctx);

    if (ffmpeg.avformat_find_stream_info(fmt_ctx, null) < 0) {
        return error.CouldNotFindStreamInfo;
    }

    // 4. 查找最佳音频流
    const audio_stream_idx = ffmpeg.av_find_best_stream(fmt_ctx, ffmpeg.AVMEDIA_TYPE_AUDIO, -1, -1, null, 0);
    if (audio_stream_idx < 0) {
        return error.NoAudioStream;
    }
    const stream = fmt_ctx.?.streams[@intCast(audio_stream_idx)];
    const codec_par = stream.*.codecpar;

    // 5. 初始化解码器
    const codec = ffmpeg.avcodec_find_decoder(codec_par.*.codec_id);
    if (codec == null) return error.DecoderNotFound;

    var codec_ctx = ffmpeg.avcodec_alloc_context3(codec);
    defer ffmpeg.avcodec_free_context(@ptrCast(&codec_ctx));

    if (ffmpeg.avcodec_parameters_to_context(codec_ctx, codec_par) < 0) return error.CodecParamCopyFailed;
    if (ffmpeg.avcodec_open2(codec_ctx, codec, null) < 0) return error.CodecOpenFailed;

    // 6. 初始化重采样器 (SwrContext)
    // 目标格式：SDL3 最喜欢的 Float32, 双声道, 48000Hz
    const dst_sample_rate = 48000;
    const dst_sample_fmt = ffmpeg.AV_SAMPLE_FMT_FLT; // Interleaved Float
    const dst_channels = 2;

    var dst_ch_layout = ffmpeg.AVChannelLayout{};
    ffmpeg.av_channel_layout_default(&dst_ch_layout, dst_channels);

    var swr_ctx = ffmpeg.swr_alloc();
    defer ffmpeg.swr_free(&swr_ctx);

    // 设置输入参数 (来自解码器上下文)
    _ = ffmpeg.av_opt_set_chlayout(swr_ctx, "in_chlayout", &codec_ctx.*.ch_layout, 0);
    _ = ffmpeg.av_opt_set_int(swr_ctx, "in_sample_rate", codec_ctx.*.sample_rate, 0);
    _ = ffmpeg.av_opt_set_sample_fmt(swr_ctx, "in_sample_fmt", codec_ctx.*.sample_fmt, 0);

    // 设置输出参数 (我们想要的 SDL 格式)
    _ = ffmpeg.av_opt_set_chlayout(swr_ctx, "out_chlayout", &dst_ch_layout, 0);
    _ = ffmpeg.av_opt_set_int(swr_ctx, "out_sample_rate", dst_sample_rate, 0);
    _ = ffmpeg.av_opt_set_sample_fmt(swr_ctx, "out_sample_fmt", dst_sample_fmt, 0);

    if (ffmpeg.swr_init(swr_ctx) < 0) {
        return error.SwrInitFailed;
    }

    // 7. 初始化 SDL3 Audio Stream
    const audio_spec = c.SDL_AudioSpec{
        .format = c.SDL_AUDIO_F32, // 对应 AV_SAMPLE_FMT_FLT
        .channels = dst_channels,
        .freq = dst_sample_rate,
    };

    // 打开默认播放设备并创建一个流
    const stream_handle = c.SDL_OpenAudioDeviceStream(c.SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK, &audio_spec, null, null);
    if (stream_handle == null) {
        std.debug.print("SDL_OpenAudioDeviceStream failed: {s}\n", .{c.SDL_GetError()});
        return error.SDLOpenAudioFailed;
    }
    defer c.SDL_DestroyAudioStream(stream_handle);

    // 开始播放 (此时流是空的，会播静音)
    const device_id = c.SDL_GetAudioStreamDevice(stream_handle);
    _ = c.SDL_ResumeAudioDevice(device_id);

    std.debug.print("Starting playback... Press Ctrl+C to stop.\n", .{});

    // 8. 主循环：读取 -> 解码 -> 重采样 -> 推送
    var packet = ffmpeg.av_packet_alloc();
    defer ffmpeg.av_packet_free(&packet);

    var frame = ffmpeg.av_frame_alloc();
    defer ffmpeg.av_frame_free(&frame);

    // 预分配一点 buffer 给重采样输出用 (4096 samples usually enough for a frame)
    // 实际上应该根据 frame_size 动态计算，这里为了 POC 简化
    var out_buf: [*c]u8 = null;
    var out_linesize: c_int = 0;
    // 先分配足够大的空间 (比如 1秒的数据)，避免循环里反复 alloc
    _ = ffmpeg.av_samples_alloc(&out_buf, &out_linesize, dst_channels, 48000, dst_sample_fmt, 0);
    defer ffmpeg.av_freep(@ptrCast(&out_buf)); // 注意用 av_freep 释放

    // 在循环开始前获取总时长（秒）
    // ffmpeg.AV_TIME_BASE 是 1,000,000
    const total_duration = @as(f64, @floatFromInt(fmt_ctx.?.duration)) / @as(
        f64,
        @floatFromInt(ffmpeg.AV_TIME_BASE),
    );
    var last_print_time: i64 = 0;
    var event: c.SDL_Event = undefined;
    while (ffmpeg.av_read_frame(fmt_ctx, packet) >= 0) {
        // 允许 SDL 处理系统事件
        while (c.SDL_PollEvent(&event)) {
            if (event.type == c.SDL_EVENT_QUIT) return; // 响应关闭信号
        }
        if (packet.*.stream_index == audio_stream_idx) {

            // --- 进度计算 ---
            // 获取当前 packet 的时间戳并转换为秒
            // stream.*.time_base 决定了 pts 的单位
            const current_real_time = std.time.milliTimestamp();

            // 只有达到间隔时间才打印
            if (current_real_time - last_print_time > 100) {
                const current_pts = @as(f64, @floatFromInt(packet.*.pts));
                const time_base = @as(f64, @floatFromInt(stream.*.time_base.num)) / @as(f64, @floatFromInt(stream.*.time_base.den));
                const current_time = current_pts * time_base;

                try printProgressBar(current_time, total_duration);
                last_print_time = current_real_time;
            }

            // 发送给解码器
            if (ffmpeg.avcodec_send_packet(codec_ctx, packet) == 0) {
                // 接收解码后的帧 (一个 packet 可能包含多帧)
                while (ffmpeg.avcodec_receive_frame(codec_ctx, frame) == 0) {

                    // --- 流量控制 (Flow Control) ---
                    // 检查 SDL 缓冲区里还有多少数据
                    // 如果堆积了太多 (比如超过 0.5秒)，就睡一会儿，防止内存爆掉
                    // 4 bytes (float) * 2 channels * 48000 rate * 0.5 sec = ~192KB
                    const max_queued_bytes = 192000;
                    while (c.SDL_GetAudioStreamQueued(stream_handle) > max_queued_bytes) {
                        sleep(10 * std.time.ns_per_ms);
                    }

                    // --- 重采样 ---
                    // 计算输出样本数 (加上一点 buffer 应对重采样延迟)
                    const delay = ffmpeg.swr_get_delay(swr_ctx, codec_ctx.*.sample_rate);
                    const dst_nb_samples = ffmpeg.av_rescale_rnd(delay + frame.*.nb_samples, dst_sample_rate, codec_ctx.*.sample_rate, ffmpeg.AV_ROUND_UP);

                    // 确保 out_buf 够大，不够大要 realloc (这里 POC 假设够大，或者你可以加逻辑检查)
                    // ...

                    // 执行转换
                    // swr_convert 返回实际转换出的样本数
                    const samples_converted = ffmpeg.swr_convert(swr_ctx, &out_buf, @intCast(dst_nb_samples), @ptrCast(&frame.*.data), // 输入数据
                        frame.*.nb_samples);

                    if (samples_converted > 0) {
                        // --- 推送给 SDL ---
                        // 计算字节数: samples * channels * sizeof(float)
                        const data_size = samples_converted * dst_channels * @sizeOf(f32);

                        if (c.SDL_PutAudioStreamData(stream_handle, out_buf, @intCast(data_size)) == false) {
                            std.debug.print("SDL_PutAudioStreamData failed: {s}\n", .{c.SDL_GetError()});
                        }
                    }
                }
            }
        }
        ffmpeg.av_packet_unref(packet);
    }

    std.debug.print("Playback finished.\n", .{});
    // 等待 SDL 播完缓冲区里剩余的数据
    while (c.SDL_GetAudioStreamQueued(stream_handle) > 0) {
        sleep(100 * std.time.ns_per_ms);
    }
}

fn printProgressBar(current: f64, total: f64) !void {
    const width = 40; // 进度条总字符宽度
    const percentage = @min(1.0, current / total);
    const filled_width = @as(usize, @intFromFloat(percentage * @as(f64, width)));

    // 构建进度条字符串
    // \r 会让光标回到行首，\x1b[K 会清除从光标到行末的内容（ANSI 转义码）
    std.debug.print("\r\x1b[K[", .{});

    var i: usize = 0;
    while (i < width) : (i += 1) {
        if (i < filled_width) {
            std.debug.print("=", .{});
        } else if (i == filled_width) {
            std.debug.print(">", .{});
        } else {
            std.debug.print(" ", .{});
        }
    }

    std.debug.print("] {d:3.1}% ({d:>.2}s / {d:>.2}s)", .{ percentage * 100.0, current, total });
}
