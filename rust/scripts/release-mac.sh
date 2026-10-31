#!/bin/bash
# Builds the editor in release mode and packages it as dist/OpenCut.app.
# FFmpeg and every non-system library it loads are copied into Contents/Frameworks and
# relinked through @rpath, so the app runs without the vendored FFmpeg or Homebrew.
set -euo pipefail

rust_root="$(cd -- "$(dirname -- "$0")/.." && pwd)"
cd -- "$rust_root"

ffmpeg_dir="$rust_root/vendor/ffmpeg-8.1.2"
if [[ ! -f "$ffmpeg_dir/lib/pkgconfig/libavcodec.pc" ]]; then
    echo "The vendored FFmpeg installation is missing: $ffmpeg_dir" >&2
    exit 1
fi

version="$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)"
app="$rust_root/dist/OpenCut.app"
contents="$app/Contents"
frameworks="$contents/Frameworks"

cargo build --release --config .cargo/macos.toml -p editor

rm -rf "$app"
mkdir -p "$contents/MacOS" "$frameworks" "$contents/Resources/FFmpeg"
cp target/release/editor "$contents/MacOS/OpenCut"
chmod u+w "$contents/MacOS/OpenCut"

# Prints the file that `reference`, as written in the load commands of `origin`, points to.
resolve_reference() {
    local origin="$1" reference="$2" rpath candidate
    case "$reference" in
        @loader_path/*)
            echo "$(dirname -- "$origin")/${reference#@loader_path/}"
            ;;
        @rpath/*)
            while read -r rpath; do
                candidate="${rpath/@loader_path/$(dirname -- "$origin")}/${reference#@rpath/}"
                if [[ -f "$candidate" ]]; then
                    echo "$candidate"
                    return
                fi
            done < <(otool -l "$origin" | awk '/LC_RPATH/ { getline; getline; print $2 }')
            echo "Could not resolve $reference from $origin" >&2
            exit 1
            ;;
        *)
            echo "$reference"
            ;;
    esac
}

# Copies the non-system libraries that `target` loads into Frameworks, recursively, and
# rewrites its references to @rpath. `origin` is where `target` was copied from, for
# resolving @loader_path and @rpath references.
bundle_dependencies() {
    local target="$1" origin="$2" reference source name own_id
    own_id="$(otool -D "$target" | tail -n +2)" # 动态库自己的 install name；可执行文件为空
    for reference in $(otool -L "$target" | tail -n +2 | awk '{ print $1 }'); do
        case "$reference" in
            /System/* | /usr/lib/*) continue ;;
        esac
        if [[ "$reference" == "$own_id" ]]; then
            continue
        fi
        source="$(realpath "$(resolve_reference "$origin" "$reference")")"
        name="$(basename -- "$source")" # 符号链接解析到真实文件，同一个库只拷贝一份
        install_name_tool -change "$reference" "@rpath/$name" "$target" 2>/dev/null
        if [[ ! -f "$frameworks/$name" ]]; then
            cp "$source" "$frameworks/$name"
            chmod u+w "$frameworks/$name"
            install_name_tool -id "@rpath/$name" "$frameworks/$name" 2>/dev/null
            while read -r rpath; do
                install_name_tool -delete_rpath "$rpath" "$frameworks/$name" 2>/dev/null
            done < <(otool -l "$frameworks/$name" | awk '/LC_RPATH/ { getline; getline; print $2 }')
            bundle_dependencies "$frameworks/$name" "$source"
        fi
    done
}

bundle_dependencies "$contents/MacOS/OpenCut" "$rust_root/target/release/editor"
install_name_tool -add_rpath "@executable_path/../Frameworks" "$contents/MacOS/OpenCut"

leftover="$(otool -L "$contents/MacOS/OpenCut" "$frameworks"/*.dylib | grep -E '^\s+/(opt|Users|usr/local)/' || true)"
if [[ -n "$leftover" ]]; then
    echo "References outside the app bundle remain:" >&2
    echo "$leftover" >&2
    exit 1
fi

cp "$ffmpeg_dir"/COPYING.* "$ffmpeg_dir/LICENSE.md" "$contents/Resources/FFmpeg/"

iconset="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$iconset"
qlmanage -t -s 1024 -o "$(dirname -- "$iconset")" apps/editor/icons/app-icon.svg >/dev/null
icon_png="$(dirname -- "$iconset")/app-icon.svg.png"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$icon_png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
    sips -z $((size * 2)) $((size * 2)) "$icon_png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$contents/Resources/AppIcon.icns"

cat > "$contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>OpenCut</string>
    <key>CFBundleDisplayName</key>
    <string>OpenCut</string>
    <key>CFBundleIdentifier</key>
    <string>com.opencut.editor</string>
    <key>CFBundleExecutable</key>
    <string>OpenCut</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$version</string>
    <key>CFBundleVersion</key>
    <string>$version</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
</dict>
</plist>
PLIST

# install_name_tool invalidates signatures; Apple Silicon refuses to load unsigned code.
codesign --force --sign - "$frameworks"/*.dylib
codesign --force --sign - "$contents/MacOS/OpenCut"
codesign --force --sign - "$app"

echo "Built $app"
