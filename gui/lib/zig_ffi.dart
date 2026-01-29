import 'dart:ffi' as ffi;
import 'dart:io';
import 'package:ffi/ffi.dart';

// Typedef for the native add function signature
typedef AddNative = ffi.Int32 Function(ffi.Int32 a, ffi.Int32 b);

// Typedef for the Dart add function signature
typedef AddDart = int Function(int a, int b);

// Typedef for the native get_video_information function signature
typedef GetVideoInformationNative = ffi.Pointer<ffi.Char> Function(ffi.Pointer<ffi.Char> filePath);

// Typedef for the Dart get_video_information function signature
typedef GetVideoInformationDart = ffi.Pointer<ffi.Char> Function(ffi.Pointer<ffi.Char> filePath);

// Typedef for the native free_string function signature
typedef FreeStringNative = ffi.Void Function(ffi.Pointer<ffi.Char> str);

// Typedef for the Dart free_string function signature
typedef FreeStringDart = void Function(ffi.Pointer<ffi.Char> str);

class ZigFFI {
  late final ffi.DynamicLibrary _lib;
  late final AddDart add;
  late final GetVideoInformationDart _getVideoInformationNative;
  late final FreeStringDart _freeStringNative;

  ZigFFI() {
    // Load the dynamic library
    _lib = _loadLibrary();

    // Look up the add function
    add = _lib
        .lookup<ffi.NativeFunction<AddNative>>('add')
        .asFunction<AddDart>();

    // Look up the get_video_information function
    _getVideoInformationNative = _lib
        .lookup<ffi.NativeFunction<GetVideoInformationNative>>('get_video_information')
        .asFunction<GetVideoInformationDart>();

    // Look up the free_string function
    _freeStringNative = _lib
        .lookup<ffi.NativeFunction<FreeStringNative>>('free_string')
        .asFunction<FreeStringDart>();
  }

  /// Get video information from an MP4 file
  /// Returns a string with video metadata or null if there's an error
  String? getVideoInformation(String filePath) {
    // Convert Dart string to C string
    final filePathPtr = filePath.toNativeUtf8();

    try {
      // Call the native function
      final resultPtr = _getVideoInformationNative(filePathPtr.cast<ffi.Char>());

      // Check if the result is null
      if (resultPtr.address == 0) {
        return null;
      }

      // Convert C string to Dart string
      final result = resultPtr.cast<Utf8>().toDartString();

      // Free the C string
      _freeStringNative(resultPtr);

      return result;
    } finally {
      // Free the input string
      malloc.free(filePathPtr);
    }
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
