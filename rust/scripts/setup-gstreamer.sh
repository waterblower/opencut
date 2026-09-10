#!/bin/bash
# Download and relocate the official macOS GStreamer runtime and SDK locally.
# Compatible with the Bash 3.2 shipped by macOS. No installer scripts are run.
set -Eeuo pipefail
trap 'printf "%s:%s: command failed: %s\n" "${BASH_SOURCE[0]}" "$LINENO" "$BASH_COMMAND" >&2' ERR

VERSION=1.28.6
SOURCE="https://gstreamer.freedesktop.org/data/pkg/osx/$VERSION"
ORIGINAL=/Library/Frameworks/GStreamer.framework
script_dir="$(cd -- "$(dirname -- "$0")" && pwd)"
VENDOR="$(cd -- "$script_dir/.." && pwd)/vendor/gstreamer"
DESTINATION="$VENDOR/GStreamer.framework"
PACKAGES=(
    "gstreamer-1.0-$VERSION-universal.pkg"
    "gstreamer-1.0-devel-$VERSION-universal.pkg"
)
PACKAGE_HASHES=(
    a8eb366c59b7e9e5dc049848fed6bcd203a8878aa7517c051639fda78797c6ad
    177b1428d0f47b844e7bff2aeeb22047686d802eba21580dab52f4a6fe1dcf02
)
FAAC_NAMES=(faac-2.1.tar.gz "gst-plugins-bad-$VERSION.tar.xz" faac-2-api.diff)
FAAC_URLS=(
    https://github.com/knik0/faac/archive/refs/tags/faac-2.1.tar.gz
    "https://gstreamer.freedesktop.org/src/gst-plugins-bad/gst-plugins-bad-$VERSION.tar.xz"
    https://gitlab.freedesktop.org/gstreamer/gstreamer/-/commit/49b4b4129e3b488f246493d3a57dc70652ec9dcf.diff
)
FAAC_HASHES=(
    1d4b890c7d767361987d80afdacdd654d23a748b4a273d743c174c2d57e9bce5
    6636f2c2289ceda52c4aba971338c81e2b5780d3381bd3673c1c116ec87587c3
    25ef9fc417878e0aac46ffb0f16c5a5d1a44341cd3364c97111980fb5bfd64b8
)

main() {
    local faac_only=false verify_only=false argument tool index
    for argument in "$@"; do
        case "$argument" in
            --faac-only) faac_only=true ;;
            --verify-only) verify_only=true ;;
            -h|--help)
                printf 'Usage: bash scripts/setup-gstreamer.sh [--faac-only] [--verify-only]\n\n--faac-only    Add FAAC to the existing SDK.\n--verify-only  Download and verify archives without extracting or building.\n'
                return ;;
            *) fail "Unknown argument: $argument" ;;
        esac
    done
    [[ "$(uname -s)" == Darwin ]] || fail 'This setup requires macOS and Xcode command line tools'
    for tool in curl shasum pkgutil ditto otool install_name_tool codesign xcrun lipo patch pkg-config tar xmllint od find awk; do
        command -v "$tool" >/dev/null || fail "Required tool is missing: $tool"
    done
    mkdir -p "$VENDOR/downloads"
    if [[ -e "$DESTINATION" && ! -f "$VENDOR/setup.json" ]]; then
        fail "Refusing to replace an unmanaged framework: $DESTINATION"
    fi
    if $faac_only && [[ ! -d "$DESTINATION" ]]; then
        fail 'Run setup without --faac-only to download the GStreamer SDK first'
    fi
    for index in 0 1 2; do
        download "${FAAC_NAMES[$index]}" "${FAAC_URLS[$index]}" "${FAAC_HASHES[$index]}"
    done
    if ! $faac_only; then
        for index in 0 1; do
            download "${PACKAGES[$index]}" "$SOURCE/${PACKAGES[$index]}" "${PACKAGE_HASHES[$index]}"
        done
    fi
    if $verify_only; then return; fi

    # Stage all changes first. Preserve the previous SDK if installation fails.
    local work framework expanded info location relative path target ancestor suffix
    local contents relocated magic dependencies identities dependency identity required
    local changes=() binaries=()
    work="$(mktemp -d "$VENDOR/.setup-XXXXXX")"
    trap 'cleanup "$work"' EXIT
    framework="$work/GStreamer.framework"
    mkdir -p "$framework"
    if $faac_only; then
        build_faac "$DESTINATION/Versions/1.0" "$framework/Versions/1.0" "$work"
        for path in lib/libfaac.1.dylib lib/gstreamer-1.0/libgstfaac.dylib; do
            codesign --force --sign - "$framework/Versions/1.0/$path"
        done
        ditto "$framework" "$DESTINATION"
        printf 'FAAC 2.1 is ready in %s\n' "$DESTINATION"
    else
        for index in 0 1; do
            expanded="$work/package-$index"
            printf 'Extracting %s (no installer scripts are executed)\n' "${PACKAGES[$index]}"
            pkgutil --expand-full "$VENDOR/downloads/${PACKAGES[$index]}" "$expanded"
            find "$expanded" -name PackageInfo -type f -print0 > "$work/components"
            [[ -s "$work/components" ]] || fail "No component packages found in ${PACKAGES[$index]}"
            while IFS= read -r -d '' info; do
                location="$(xmllint --nonet --xpath 'string(/pkg-info/@install-location)' "$info")"
                case "$location" in
                    "$ORIGINAL") relative= ;;
                    "$ORIGINAL"/*) relative="${location#"$ORIGINAL"/}" ;;
                    *) fail "Unexpected package install location: $location" ;;
                esac
                case "/$relative/" in */../*) fail "Unsafe package install location: $location" ;; esac
                ditto "${info%/*}/Payload" "$framework/$relative"
            done < "$work/components"
            rm -rf -- "$expanded"
        done
        build_faac "$framework/Versions/1.0" "$framework/Versions/1.0" "$work"
        find "$framework" -print0 > "$work/paths"
        while IFS= read -r -d '' path; do
            if [[ -L "$path" ]]; then
                target="$(readlink "$path")"
                if [[ "$target" == "$ORIGINAL"/* ]]; then
                    target="$framework/${target#"$ORIGINAL"/}"
                    ancestor="${path%/*}"
                    suffix=
                    while [[ "$target" != "$ancestor"/* ]]; do
                        ancestor="${ancestor%/*}"
                        suffix="../$suffix"
                    done
                    rm -- "$path"
                    ln -s "$suffix${target#"$ancestor"/}" "$path"
                fi
                continue
            fi
            [[ -f "$path" && "$path" != *.a ]] || continue
            case "$path" in
                *.pc|*.la|*.cmake)
                    contents="$(cat "$path")"
                    relocated="${contents//"$ORIGINAL"/"$DESTINATION"}"
                    if [[ "$contents" != "$relocated" ]]; then printf '%s\n' "$relocated" > "$path"; fi ;;
            esac
            magic="$(od -An -N4 -tx1 "$path" | tr -d ' \n')"
            case "$magic" in
                feedface|cefaedfe|feedfacf|cffaedfe|cafebabe|bebafeca|cafebabf|bfbafeca) binaries+=("$path") ;;
            esac
        done < "$work/paths"
        printf 'Relocating and signing %s Mach-O files\n' "${#binaries[@]}"
        for path in "${binaries[@]}"; do
            identities="$(otool -D "$path" | awk 'NF && $0 !~ /:$/ { sub(/^[[:space:]]+/, ""); print }')"
            dependencies="$(otool -L "$path" | awk '/^[[:space:]]+.* \(compatibility/ { sub(/^[[:space:]]+/, ""); sub(/ \(compatibility.*/, ""); print }' | sort -u)"
            changes=()
            while IFS= read -r dependency; do
                [[ -n "$dependency" ]] || continue
                identity=false
                while IFS= read -r target; do
                    if [[ "$target" == "$dependency" ]]; then identity=true; break; fi
                done <<< "$identities"
                if $identity; then continue; fi
                case "$dependency" in
                    '@rpath/Python3.framework/Versions/3.9/Python3') continue ;;
                    '@rpath/'*) relative="Versions/1.0/lib/${dependency#@rpath/}" ;;
                    "$ORIGINAL"/*) relative="${dependency#"$ORIGINAL"/}" ;;
                    "$DESTINATION"/*) relative="${dependency#"$DESTINATION"/}" ;;
                    /usr/lib/*|/System/Library/*|'@loader_path/'*|'@executable_path/'*) continue ;;
                    *) fail "Unexpected dependency in $path: $dependency" ;;
                esac
                [[ -e "$framework/$relative" ]] || fail "Missing vendored dependency in $path: $dependency"
                changes+=(-change "$dependency" "$DESTINATION/$relative")
            done <<< "$dependencies"
            if [[ -n "$identities" ]]; then changes+=(-id "$DESTINATION/${path#"$framework"/}"); fi
            if [[ ${#changes[@]} -gt 0 ]]; then
                install_name_tool "${changes[@]}" "$path"
                codesign --force --sign - "$path"
            fi
        done
        while IFS= read -r -d '' path; do
            if [[ -L "$path" && ! -e "$path" ]]; then fail "Broken framework symlink: $path -> $(readlink "$path")"; fi
        done < "$work/paths"
        for required in lib/libgstreamer-1.0.dylib lib/libges-1.0.dylib lib/pkgconfig/gstreamer-1.0.pc lib/pkgconfig/gst-editing-services-1.0.pc libexec/gstreamer-1.0/gst-plugin-scanner lib/libfaac.1.dylib lib/gstreamer-1.0/libgstfaac.dylib; do
            [[ -f "$framework/Versions/1.0/$required" ]] || fail "Package is missing $required"
        done
        # Write valid metadata before swapping the managed framework.
        {
            printf '{"version":"%s","source":"%s","sha256":{' "$VERSION" "$SOURCE"
            printf '"%s":"%s","%s":"%s"},"framework":' "${PACKAGES[0]}" "${PACKAGE_HASHES[0]}" "${PACKAGES[1]}" "${PACKAGE_HASHES[1]}"
            json_string "$DESTINATION"
            printf '}\n'
        } > "$work/setup.json"
        if [[ -e "$DESTINATION" ]]; then mv -- "$DESTINATION" "$work/previous.framework"; fi
        mv -- "$framework" "$DESTINATION"
        mv -- "$work/setup.json" "$VENDOR/setup.json"
        printf 'GStreamer %s is ready at %s\nRun cargo editor-mac from rust/.\n' "$VERSION" "$DESTINATION"
    fi
    cleanup "$work"
    trap - EXIT
}

fail() {
    printf '%s:%s: %s\n' "${BASH_SOURCE[1]}" "${BASH_LINENO[0]}" "$*" >&2
    exit 1
}

cleanup() {
    local work="$1"
    if [[ -d "$work/previous.framework" && ! -e "$DESTINATION" ]]; then
        mv -- "$work/previous.framework" "$DESTINATION" || return
    fi
    rm -rf -- "$work"
}

json_string() {
    local value="$1"
    value="${value//\\/\\\\}"
    value="${value//\"/\\\"}"
    value="${value//$'\n'/\\n}"
    value="${value//$'\r'/\\r}"
    value="${value//$'\t'/\\t}"
    printf '"%s"' "$value"
}

download() {
    local name="$1" url="$2" expected="$3" package digest
    package="$VENDOR/downloads/$name"
    if [[ ! -f "$package" ]]; then
        curl --fail --location --retry 3 --output "$package.partial" "$url"
        mv -- "$package.partial" "$package"
    fi
    digest="$(shasum -a 256 "$package")"
    digest="${digest%% *}"
    [[ "$digest" == "$expected" ]] || fail "SHA-256 mismatch: $package; remove it and rerun setup"
    printf 'Verified %s: %s\n' "$name" "$digest"
}

build_faac() {
    local sdk="$1" output="$2" work="$3" archive architecture minimum source index
    local faac="$3/faac-faac-2.1" plugins="$3/gst-plugins-bad-$VERSION"
    local library_dir="$2/lib" plugin_dir="$2/lib/gstreamer-1.0"
    local final_lib="$DESTINATION/Versions/1.0/lib"
    local sources=() flags=() slices=() metadata flag character quote= escaped=false
    for archive in faac-2.1.tar.gz "gst-plugins-bad-$VERSION.tar.xz"; do
        tar -xf "$VENDOR/downloads/$archive" -C "$work"
    done
    (cd -- "$plugins" && patch --batch --fuzz=0 -p3 -i "$VENDOR/downloads/faac-2-api.diff")
    mkdir -p "$plugin_dir"
    for architecture in arm64 x86_64; do
        minimum=11.0
        if [[ "$architecture" == x86_64 ]]; then minimum=10.13; fi
        printf 'Building FAAC 2.1 for %s\n' "$architecture"
        sources=()
        for source in "$faac"/libfaac/*.c; do
            if [[ "${source##*/}" == quantize_sse.c && "$architecture" != x86_64 ]]; then continue; fi
            sources+=("$source")
        done
        flags=(-DMAX_CHANNELS=8 -DFAAC_SBR_DECIMATION=1 '-DPACKAGE="faac"' '-DPACKAGE_VERSION="2.1.0"')
        if [[ "$architecture" == x86_64 ]]; then flags+=(-DHAVE_SSE2=1 -msse2); fi
        xcrun clang -arch "$architecture" "-mmacosx-version-min=$minimum" \
            -std=gnu11 -O2 -DNDEBUG -fvisibility=hidden -dynamiclib -Wl,-headerpad_max_install_names \
            "${flags[@]}" -I "$faac/include" "${sources[@]}" \
            -install_name "$final_lib/libfaac.1.dylib" -compatibility_version 1.0.0 -current_version 1.0.0 \
            -o "$work/libfaac-$architecture.dylib"
        slices+=("$work/libfaac-$architecture.dylib")
    done
    lipo -create "${slices[@]}" -output "$library_dir/libfaac.1.dylib"
    ln -s libfaac.1.dylib "$library_dir/libfaac.dylib"
    metadata="$(
        export PKG_CONFIG_PATH="$sdk/lib/pkgconfig" PKG_CONFIG_LIBDIR="$sdk/lib/pkgconfig"
        unset PKG_CONFIG_SYSROOT_DIR
        pkg-config --define-variable="prefix=$sdk" --cflags --libs gstreamer-audio-1.0 gstreamer-pbutils-1.0 gstreamer-tag-1.0
    )"
    # Parse pkg-config's shell quoting without eval or executing its output.
    flags=()
    flag=
    for ((index=0; index<${#metadata}; index++)); do
        character="${metadata:index:1}"
        if $escaped; then flag+="$character"; escaped=false; continue; fi
        if [[ "$character" == '\' && "$quote" != "'" ]]; then escaped=true; continue; fi
        if [[ -n "$quote" ]]; then
            if [[ "$character" == "$quote" ]]; then quote=; else flag+="$character"; fi
            continue
        fi
        case "$character" in
            "'"|'"') quote="$character" ;;
            ' '|$'\t'|$'\n')
                if [[ -n "$flag" && "$flag" != -Wl,-rpath,* ]]; then flags+=("$flag"); fi
                flag= ;;
            *) flag+="$character" ;;
        esac
    done
    if $escaped || [[ -n "$quote" ]]; then fail 'Invalid quoting in pkg-config output'; fi
    if [[ -n "$flag" && "$flag" != -Wl,-rpath,* ]]; then flags+=("$flag"); fi
    printf 'Building the universal GStreamer FAAC plugin\n'
    xcrun clang -arch arm64 -arch x86_64 -mmacosx-version-min=10.13 -std=gnu11 -O2 -dynamiclib \
        -Wl,-headerpad_max_install_names '-DPACKAGE="gst-plugins-bad"' "-DVERSION=\"$VERSION\"" \
        '-DGST_PACKAGE_NAME="OpenCut vendored GStreamer"' '-DGST_PACKAGE_ORIGIN="https://gstreamer.freedesktop.org/"' \
        -I "$faac/include" "$plugins/ext/faac/gstfaac.c" "${flags[@]}" -L "$library_dir" -lfaac \
        -install_name "$final_lib/gstreamer-1.0/libgstfaac.dylib" -o "$plugin_dir/libgstfaac.dylib"
    mkdir -p "$output/include" "$library_dir/pkgconfig" "$output/share/licenses/opencut-faac"
    cp "$faac/include/faac.h" "$output/include/faac.h"
    cat > "$library_dir/pkgconfig/faac.pc" <<'PC'
prefix=${pcfiledir}/../..
libdir=${prefix}/lib
includedir=${prefix}/include
Name: FAAC
Description: AAC audio encoder
Version: 2.1.0
Libs: -L${libdir} -lfaac
Cflags: -I${includedir}
PC
    cp "$faac/COPYING" "$output/share/licenses/opencut-faac/FAAC-COPYING"
    cp "$plugins/COPYING" "$output/share/licenses/opencut-faac/GStreamer-COPYING"
    {
        printf '{'
        for index in 0 1 2; do
            if [[ "$index" != 0 ]]; then printf ','; fi
            printf '"%s":["%s","%s"]' "${FAAC_NAMES[$index]}" "${FAAC_URLS[$index]}" "${FAAC_HASHES[$index]}"
        done
        printf '}\n'
    } > "$output/share/licenses/opencut-faac/sources.json"
}

main "$@"
