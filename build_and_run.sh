#!/bin/bash

# Build and run Flutter app with Zig FFI library
# This script builds the Zig library and copies it to the macOS app bundle

set -e

# Get the directory where this script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

echo "Building Zig FFI library..."
zig build ffi

echo "Copying library to macOS Frameworks..."
mkdir -p gui/macos/Runner/Frameworks
cp -f zig-out/lib/libzig_ffi.dylib gui/macos/Runner/Frameworks/

echo "Running Flutter app..."
cd gui
flutter run -d macos
