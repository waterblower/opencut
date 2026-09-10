#!/usr/bin/env bash
# Local release linked to the repository's existing FFmpeg installation.
set -euo pipefail
cli_root="$(cd -- "$(dirname -- "$0")/.." && pwd)"
cli_prefix="$cli_root/vendor/ffmpeg-8.1.2"
export CARGO_TARGET_DIR="$cli_root/target/cli-release"
cli_features=cli
if [[ "${OPENCUT_GPL:-0}" == 1 ]]; then cli_features+=,gpl; fi
bash "$cli_root/scripts/cargo-cli.sh" rustc --locked --release --no-default-features --features "$cli_features" --bin opencut -- -C strip=symbols
cli_stage="$(mktemp -d "$cli_root/target/opencut-release.XXXXXX")"
cp "$CARGO_TARGET_DIR/release/opencut" "$cli_stage/opencut"
cli_size="$(stat -f %z "$cli_stage/opencut")"
[[ "$cli_size" -le 100000000 ]] || { echo "Binary exceeds 100 MB: $cli_size" >&2; exit 1; }
otool -L "$cli_stage/opencut"
cp "$cli_root/src/cli/README.md" "$cli_root/src/cli/llms.txt" "$cli_stage/"
cp "$cli_root/vendor/zed/assets/fonts/ibm-plex-sans/license.txt" "$cli_stage/font-license.txt"
cp "$cli_prefix/LICENSE.md" "$cli_prefix/COPYING.GPLv2" "$cli_prefix/COPYING.GPLv3" "$cli_prefix/COPYING.LGPLv2.1" "$cli_prefix/COPYING.LGPLv3" "$cli_stage/"
tar -czf "$cli_stage.tar.gz" -C "$cli_stage" .
echo "Local release: $cli_stage.tar.gz ($cli_size byte binary; requires the existing vendored FFmpeg and its linked dependencies)" >&2
