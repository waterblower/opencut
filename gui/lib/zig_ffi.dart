import 'dart:ffi' as ffi;
import 'dart:io';

// Typedef for the native add function signature
typedef AddNative = ffi.Int32 Function(ffi.Int32 a, ffi.Int32 b);

// Typedef for the Dart add function signature
typedef AddDart = int Function(int a, int b);

class ZigFFI {
  late final ffi.DynamicLibrary _lib;
  late final AddDart add;

  ZigFFI() {
    // Load the dynamic library
    _lib = _loadLibrary();

    // Look up the add function
    add = _lib
        .lookup<ffi.NativeFunction<AddNative>>('add')
        .asFunction<AddDart>();
  }

  ffi.DynamicLibrary _loadLibrary() {
    // Use absolute path to the library
    // Sandboxing is disabled in DebugProfile.entitlements for development
    const projectRoot = '/Users/mac/Documents/GitHub/vid-demo';

    if (Platform.isMacOS) {
      return ffi.DynamicLibrary.open('$projectRoot/zig-out/lib/libzig_ffi.dylib');
    } else if (Platform.isLinux) {
      return ffi.DynamicLibrary.open('$projectRoot/zig-out/lib/libzig_ffi.so');
    } else if (Platform.isWindows) {
      return ffi.DynamicLibrary.open('$projectRoot/zig-out/lib/zig_ffi.dll');
    } else {
      throw UnsupportedError('Unsupported platform');
    }
  }
}
