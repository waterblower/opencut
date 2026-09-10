#!/usr/bin/env bash
set -euo pipefail
cli_root="$(cd -- "$(dirname -- "$0")/.." && pwd)"
cli_ffmpeg="$cli_root/vendor/ffmpeg-8.1.2"
if [[ ! -f "$cli_ffmpeg/lib/pkgconfig/libavcodec.pc" ]]; then
    echo "The vendored FFmpeg installation is missing: $cli_ffmpeg" >&2
    exit 1
fi
export PKG_CONFIG_PATH="$cli_ffmpeg/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export DYLD_FALLBACK_LIBRARY_PATH="$cli_ffmpeg/lib${DYLD_FALLBACK_LIBRARY_PATH:+:$DYLD_FALLBACK_LIBRARY_PATH}:/usr/lib"
export LD_LIBRARY_PATH="$cli_ffmpeg/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
unset FFMPEG_DIR
cd -- "$cli_root"
if [[ "${1:-}" == --exec ]]; then
    shift
    exec "$@"
fi
exec cargo "$@"
