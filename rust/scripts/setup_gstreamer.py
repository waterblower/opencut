#!/usr/bin/env python3
"""Download and relocate the official macOS GStreamer runtime and SDK locally."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET


VERSION = "1.28.6"
PACKAGES = {
    f"gstreamer-1.0-{VERSION}-universal.pkg":
        "a8eb366c59b7e9e5dc049848fed6bcd203a8878aa7517c051639fda78797c6ad",
    f"gstreamer-1.0-devel-{VERSION}-universal.pkg":
        "177b1428d0f47b844e7bff2aeeb22047686d802eba21580dab52f4a6fe1dcf02",
}
SOURCE = f"https://gstreamer.freedesktop.org/data/pkg/osx/{VERSION}"
FAAC_SOURCES = {
    "faac-2.1.tar.gz": (
        "https://github.com/knik0/faac/archive/refs/tags/faac-2.1.tar.gz",
        "1d4b890c7d767361987d80afdacdd654d23a748b4a273d743c174c2d57e9bce5",
    ),
    f"gst-plugins-bad-{VERSION}.tar.xz": (
        f"https://gstreamer.freedesktop.org/src/gst-plugins-bad/gst-plugins-bad-{VERSION}.tar.xz",
        "6636f2c2289ceda52c4aba971338c81e2b5780d3381bd3673c1c116ec87587c3",
    ),
    "faac-2-api.diff": (
        "https://gitlab.freedesktop.org/gstreamer/gstreamer/-/commit/49b4b4129e3b488f246493d3a57dc70652ec9dcf.diff",
        "25ef9fc417878e0aac46ffb0f16c5a5d1a44341cd3364c97111980fb5bfd64b8",
    ),
}
ORIGINAL = Path("/Library/Frameworks/GStreamer.framework")
VENDOR = Path(__file__).resolve().parents[1] / "vendor/gstreamer"
DESTINATION = VENDOR / "GStreamer.framework"
# Mach-O, fat Mach-O, and their byte-swapped variants; do not modify static archives.
MACHO_MAGICS = {
    b"\xfe\xed\xfa\xce", b"\xce\xfa\xed\xfe",
    b"\xfe\xed\xfa\xcf", b"\xcf\xfa\xed\xfe",
    b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca",
    b"\xca\xfe\xba\xbf", b"\xbf\xba\xfe\xca",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--faac-only", action="store_true", help="Add FAAC to an existing vendored framework")
    arguments = parser.parse_args()
    if sys.version_info < (3, 11):
        raise RuntimeError("GStreamer setup requires Python 3.11 or newer")
    if platform.system() != "Darwin":
        raise RuntimeError("This setup requires macOS and Xcode command line tools")
    for tool in ("curl", "pkgutil", "ditto", "otool", "install_name_tool", "codesign", "xcrun", "lipo", "patch", "pkg-config", "tar"):
        if shutil.which(tool) is None:
            raise RuntimeError(f"Required tool is missing: {tool}")

    downloads = VENDOR / "downloads"
    downloads.mkdir(parents=True, exist_ok=True)
    if DESTINATION.exists() and not (VENDOR / "setup.json").is_file():
        raise RuntimeError(f"Refusing to replace an unmanaged framework: {DESTINATION}")
    if arguments.faac_only and not DESTINATION.is_dir():
        raise RuntimeError("Run setup without --faac-only to download the GStreamer SDK first")

    artifacts = dict(FAAC_SOURCES)
    if not arguments.faac_only:
        for name, expected in PACKAGES.items():
            artifacts[name] = (f"{SOURCE}/{name}", expected)
    for name, (url, expected) in artifacts.items():
        package = downloads / name
        if not package.exists():
            partial = downloads / f"{name}.partial"
            subprocess.run([
                "curl", "--fail", "--location", "--retry", "3",
                "--output", str(partial), url,
            ], check=True)
            partial.rename(package)
        with package.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        if digest != expected:
            raise RuntimeError(f"SHA-256 mismatch: {package}; remove it and rerun setup")
        print(f"Verified {name}: {digest}", flush=True)

    # Work on fresh payloads on every run, including after moving the checkout.
    with tempfile.TemporaryDirectory(prefix=".setup-", dir=VENDOR) as temporary:
        work = Path(temporary)
        framework = work / "GStreamer.framework"
        framework.mkdir()
        if arguments.faac_only:
            build_faac(DESTINATION / "Versions/1.0", framework / "Versions/1.0", work, downloads)
            for binary in (
                framework / "Versions/1.0/lib/libfaac.1.dylib",
                framework / "Versions/1.0/lib/gstreamer-1.0/libgstfaac.dylib",
            ):
                subprocess.run(["codesign", "--force", "--sign", "-", str(binary)], check=True, capture_output=True)
            subprocess.run(["ditto", str(framework), str(DESTINATION)], check=True)
            print(f"FAAC 2.1 is ready in {DESTINATION}")
            return
        for index, name in enumerate(PACKAGES):
            expanded = work / f"package-{index}"
            print(f"Extracting {name} (no installer scripts are executed)", flush=True)
            subprocess.run([
                "pkgutil", "--expand-full", str(downloads / name), str(expanded),
            ], check=True)
            infos = sorted(expanded.rglob("PackageInfo"))
            if not infos:
                raise RuntimeError(f"No component packages found in {name}")
            for info in infos:
                location = Path(ET.parse(info).getroot().attrib["install-location"])
                relative = location.relative_to(ORIGINAL)
                subprocess.run([
                    "ditto", str(info.parent / "Payload"), str(framework / relative),
                ], check=True)
            shutil.rmtree(expanded)

        build_faac(framework / "Versions/1.0", framework / "Versions/1.0", work, downloads)
        binaries = []
        for path in sorted(framework.rglob("*")):
            if path.is_symlink():
                target = os.readlink(path)
                if target.startswith(str(ORIGINAL)):
                    relocated = framework / Path(target).relative_to(ORIGINAL)
                    path.unlink()
                    path.symlink_to(os.path.relpath(relocated, path.parent))
                continue
            if not path.is_file() or path.suffix == ".a":
                continue
            if path.suffix in (".pc", ".la", ".cmake"):
                contents = path.read_text()
                relocated = contents.replace(str(ORIGINAL), str(DESTINATION))
                if relocated != contents:
                    path.write_text(relocated)
            with path.open("rb") as stream:
                magic = stream.read(4)
            if magic in MACHO_MAGICS:
                binaries.append(path)

        print(f"Relocating and signing {len(binaries)} Mach-O files", flush=True)
        for path in binaries:
            install_names = subprocess.run([
                "otool", "-D", str(path),
            ], check=True, capture_output=True, text=True).stdout
            identities = set()
            for line in install_names.splitlines():
                if line.strip() and not line.endswith(":"):
                    identities.add(line.strip())
            dependencies = subprocess.run([
                "otool", "-L", str(path),
            ], check=True, capture_output=True, text=True).stdout
            changes = []
            for dependency in sorted(set(re.findall(r"^\s+(.+?) \(compatibility", dependencies, re.M))):
                if dependency in identities:
                    continue
                # Upstream ships Python bindings but not their Python framework.
                # Preserve that optional dependency; the Rust editor does not use it.
                if dependency == "@rpath/Python3.framework/Versions/3.9/Python3":
                    continue
                if dependency.startswith("@rpath/"):
                    relative = Path("Versions/1.0/lib") / dependency.removeprefix("@rpath/")
                elif dependency.startswith(str(ORIGINAL) + "/"):
                    relative = Path(dependency).relative_to(ORIGINAL)
                elif dependency.startswith(str(DESTINATION) + "/"):
                    relative = Path(dependency).relative_to(DESTINATION)
                elif dependency.startswith(("/usr/lib/", "/System/Library/", "@loader_path/", "@executable_path/")):
                    continue
                else:
                    raise RuntimeError(f"Unexpected dependency in {path}: {dependency}")
                if not (framework / relative).exists():
                    raise RuntimeError(f"Missing vendored dependency in {path}: {dependency}")
                changes.extend(["-change", dependency, str(DESTINATION / relative)])
            if identities:
                changes.extend(["-id", str(DESTINATION / path.relative_to(framework))])
            if changes:
                subprocess.run(["install_name_tool", *changes, str(path)], check=True, capture_output=True)
                subprocess.run([
                    "codesign", "--force", "--sign", "-", str(path),
                ], check=True, capture_output=True)

        for path in framework.rglob("*"):
            if path.is_symlink() and not path.exists():
                raise RuntimeError(f"Broken framework symlink: {path} -> {os.readlink(path)}")
        for required in (
            "lib/libgstreamer-1.0.dylib", "lib/libges-1.0.dylib",
            "lib/pkgconfig/gstreamer-1.0.pc", "lib/pkgconfig/gst-editing-services-1.0.pc",
            "libexec/gstreamer-1.0/gst-plugin-scanner",
            "lib/libfaac.1.dylib", "lib/gstreamer-1.0/libgstfaac.dylib",
        ):
            if not (framework / "Versions/1.0" / required).is_file():
                raise RuntimeError(f"Package is missing {required}")

        previous = work / "previous.framework"
        if DESTINATION.exists():
            DESTINATION.rename(previous)
        try:
            framework.rename(DESTINATION)
        except OSError:
            if previous.exists():
                previous.rename(DESTINATION)
            raise
        (VENDOR / "setup.json").write_text(json.dumps({
            "version": VERSION,
            "source": SOURCE,
            "sha256": PACKAGES,
            "framework": str(DESTINATION),
        }, indent=2) + "\n")
    print(f"GStreamer {VERSION} is ready at {DESTINATION}\nRun cargo editor from rust/.")


def build_faac(sdk, output, work, downloads):
    # Build only the missing encoder and plugin, against the official SDK.
    # Use the same FAAC 2 compatibility patch as Homebrew, without its binaries.
    for archive in ("faac-2.1.tar.gz", f"gst-plugins-bad-{VERSION}.tar.xz"):
        subprocess.run(["tar", "-xf", str(downloads / archive), "-C", str(work)], check=True)
    faac = work / "faac-faac-2.1"
    plugins = work / f"gst-plugins-bad-{VERSION}"
    subprocess.run([
        "patch", "--batch", "--fuzz=0", "-p3", "-i", str(downloads / "faac-2-api.diff"),
    ], cwd=plugins, check=True, capture_output=True)

    library_dir = output / "lib"
    plugin_dir = library_dir / "gstreamer-1.0"
    plugin_dir.mkdir(parents=True, exist_ok=True)
    final_lib = DESTINATION / "Versions/1.0/lib"
    slices = []
    for architecture, minimum in (("arm64", "11.0"), ("x86_64", "10.13")):
        print(f"Building FAAC 2.1 for {architecture}", flush=True)
        sources = []
        for source in sorted((faac / "libfaac").glob("*.c")):
            if source.name == "quantize_sse.c" and architecture != "x86_64":
                continue
            sources.append(str(source))
        # Match the upstream Meson defaults: 8 channels, full-quality SBR,
        # little endian, and SSE2 only on Intel. No FAAC CLI or Meson dependency.
        flags = ["-DMAX_CHANNELS=8", "-DFAAC_SBR_DECIMATION=1", '-DPACKAGE="faac"', '-DPACKAGE_VERSION="2.1.0"']
        if architecture == "x86_64":
            flags.extend(["-DHAVE_SSE2=1", "-msse2"])
        binary = work / f"libfaac-{architecture}.dylib"
        subprocess.run([
            "xcrun", "clang", "-arch", architecture, f"-mmacosx-version-min={minimum}",
            "-std=gnu11", "-O2", "-DNDEBUG", "-fvisibility=hidden", "-dynamiclib",
            "-Wl,-headerpad_max_install_names",
            *flags, "-I", str(faac / "include"), *sources,
            "-install_name", str(final_lib / "libfaac.1.dylib"),
            "-compatibility_version", "1.0.0", "-current_version", "1.0.0",
            "-o", str(binary),
        ], check=True, capture_output=True)
        slices.append(str(binary))
    subprocess.run([
        "lipo", "-create", *slices, "-output", str(library_dir / "libfaac.1.dylib"),
    ], check=True)
    (library_dir / "libfaac.dylib").symlink_to("libfaac.1.dylib")

    environment = dict(os.environ)
    environment["PKG_CONFIG_PATH"] = str(sdk / "lib/pkgconfig")
    environment["PKG_CONFIG_LIBDIR"] = environment["PKG_CONFIG_PATH"]
    environment.pop("PKG_CONFIG_SYSROOT_DIR", None)
    metadata = subprocess.run([
        "pkg-config", "--cflags", "--libs", "gstreamer-audio-1.0",
        "gstreamer-pbutils-1.0", "gstreamer-tag-1.0",
    ], env=environment, check=True, capture_output=True, text=True).stdout
    # The final binaries use absolute dependency paths, so omit staging rpaths.
    flags = []
    for flag in shlex.split(metadata):
        if not flag.startswith("-Wl,-rpath,"):
            flags.append(flag)
    print("Building the universal GStreamer FAAC plugin", flush=True)
    subprocess.run([
        "xcrun", "clang", "-arch", "arm64", "-arch", "x86_64",
        "-mmacosx-version-min=10.13", "-std=gnu11", "-O2", "-dynamiclib",
        "-Wl,-headerpad_max_install_names",
        '-DPACKAGE="gst-plugins-bad"', f'-DVERSION="{VERSION}"',
        '-DGST_PACKAGE_NAME="OpenCut vendored GStreamer"',
        '-DGST_PACKAGE_ORIGIN="https://gstreamer.freedesktop.org/"',
        "-I", str(faac / "include"), str(plugins / "ext/faac/gstfaac.c"),
        *flags, "-L", str(library_dir), "-lfaac",
        "-install_name", str(final_lib / "gstreamer-1.0/libgstfaac.dylib"),
        "-o", str(plugin_dir / "libgstfaac.dylib"),
    ], check=True, capture_output=True)

    (output / "include").mkdir(exist_ok=True)
    shutil.copy2(faac / "include/faac.h", output / "include/faac.h")
    metadata_dir = library_dir / "pkgconfig"
    metadata_dir.mkdir(exist_ok=True)
    (metadata_dir / "faac.pc").write_text(
        "prefix=${pcfiledir}/../..\nlibdir=${prefix}/lib\nincludedir=${prefix}/include\n"
        "Name: FAAC\nDescription: AAC audio encoder\nVersion: 2.1.0\n"
        "Libs: -L${libdir} -lfaac\nCflags: -I${includedir}\n"
    )
    licenses = output / "share/licenses/opencut-faac"
    licenses.mkdir(parents=True, exist_ok=True)
    shutil.copy2(faac / "COPYING", licenses / "FAAC-COPYING")
    shutil.copy2(plugins / "COPYING", licenses / "GStreamer-COPYING")
    (licenses / "sources.json").write_text(json.dumps(FAAC_SOURCES, indent=2) + "\n")


if __name__ == "__main__":
    # Keep the traceback: setup failures include the source filename and line.
    try:
        main()
    except subprocess.CalledProcessError as error:
        if error.stderr:
            print(error.stderr.decode(errors="replace") if isinstance(error.stderr, bytes) else error.stderr, file=sys.stderr)
        raise
