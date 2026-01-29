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

// FrameData structure matching the Zig struct
final class FrameData extends ffi.Struct {
  external ffi.Pointer<ffi.Uint8> data;
  @ffi.Int32()
  external int width;
  @ffi.Int32()
  external int height;
  @ffi.Int32()
  external int size;
}

// Typedef for the native get_nth_frame function signature
typedef GetNthFrameNative = ffi.Pointer<FrameData> Function(ffi.Pointer<ffi.Char> filePath, ffi.Int32 n);

// Typedef for the Dart get_nth_frame function signature
typedef GetNthFrameDart = ffi.Pointer<FrameData> Function(ffi.Pointer<ffi.Char> filePath, int n);

// Typedef for the native free_frame_data function signature
typedef FreeFrameDataNative = ffi.Void Function(ffi.Pointer<FrameData> frameData);

// Typedef for the Dart free_frame_data function signature
typedef FreeFrameDataDart = void Function(ffi.Pointer<FrameData> frameData);

class ZigFFI {
  late final ffi.DynamicLibrary _lib;
  late final AddDart add;
  late final GetVideoInformationDart _getVideoInformationNative;
  late final FreeStringDart _freeStringNative;
  late final GetNthFrameDart _getNthFrameNative;
  late final FreeFrameDataDart _freeFrameDataNative;

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

    // Look up the get_nth_frame function
    _getNthFrameNative = _lib
        .lookup<ffi.NativeFunction<GetNthFrameNative>>('get_nth_frame')
        .asFunction<GetNthFrameDart>();

    // Look up the free_frame_data function
    _freeFrameDataNative = _lib
        .lookup<ffi.NativeFunction<FreeFrameDataNative>>('free_frame_data')
        .asFunction<FreeFrameDataDart>();
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

  /// Get the nth frame from a video file as RGB24 data
  /// Returns a list of bytes containing RGB24 pixel data, or null if there's an error
  /// Also returns the width and height of the frame
  (List<int>?, int, int)? getNthFrame(String filePath, int frameNumber) {
    // Convert Dart string to C string
    final filePathPtr = filePath.toNativeUtf8();

    try {
      // Call the native function
      final frameDataPtr = _getNthFrameNative(filePathPtr.cast<ffi.Char>(), frameNumber);

      // Check if the result is null
      if (frameDataPtr.address == 0) {
        return null;
      }

      // Extract frame data
      final frameData = frameDataPtr.ref;
      final width = frameData.width;
      final height = frameData.height;
      final size = frameData.size;

      // Copy the data to a Dart list
      final data = frameData.data.asTypedList(size);
      final dataCopy = List<int>.from(data);

      // Free the frame data
      _freeFrameDataNative(frameDataPtr);

      return (dataCopy, width, height);
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
