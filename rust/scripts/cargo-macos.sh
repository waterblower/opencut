#!/bin/bash
set -eu

script_dir="$(cd -- "$(dirname -- "$0")" && pwd)"
rust_root="$(cd -- "$script_dir/.." && pwd)"
gst_root="$rust_root/vendor/gstreamer/GStreamer.framework/Versions/1.0"
if [[ -z "${FFMPEG_DIR:-}" ]]; then
    FFMPEG_DIR="$rust_root/vendor/ffmpeg-8.1.2"
    export FFMPEG_DIR
fi
export PATH="$gst_root/bin:$FFMPEG_DIR/bin:$PATH"
export DYLD_FALLBACK_LIBRARY_PATH="$gst_root/lib:$FFMPEG_DIR/lib${DYLD_FALLBACK_LIBRARY_PATH:+:$DYLD_FALLBACK_LIBRARY_PATH}:/usr/local/lib:/usr/lib"
export PKG_CONFIG="$script_dir/gstreamer-pkg-config"
export GST_PLUGIN_PATH_1_0=""
export GST_PLUGIN_SYSTEM_PATH_1_0="$gst_root/lib/gstreamer-1.0"
export GST_PLUGIN_SCANNER_1_0="$gst_root/libexec/gstreamer-1.0/gst-plugin-scanner"
export GST_REGISTRY_1_0="$rust_root/vendor/gstreamer/registry-macos.bin"
export GIO_EXTRA_MODULES="$gst_root/lib/gio/modules"
cd -- "$rust_root"
if [[ "${1:-}" == --exec ]]; then
    shift
    exec "$@"
fi
exec cargo "$@"
